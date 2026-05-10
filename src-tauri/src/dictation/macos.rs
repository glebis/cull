use std::ptr::NonNull;
use std::sync::Mutex;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::AllocAnyThread;
use objc2_avf_audio::{AVAudioEngine, AVAudioPCMBuffer, AVAudioTime};
use objc2_foundation::{NSError, NSLocale, NSString};
use objc2_speech::{
    SFSpeechAudioBufferRecognitionRequest, SFSpeechRecognitionResult,
    SFSpeechRecognitionTask, SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize)]
struct DictationResult {
    text: String,
    is_final: bool,
}

#[derive(Clone, Serialize)]
struct DictationError {
    message: String,
}

struct DictationSession {
    engine: Retained<AVAudioEngine>,
    request: Retained<SFSpeechAudioBufferRecognitionRequest>,
    task: Retained<SFSpeechRecognitionTask>,
}

// Safety: We protect all access through a Mutex and only interact with these
// objects from the thread that created them (via the Mutex serialization).
unsafe impl Send for DictationSession {}
unsafe impl Sync for DictationSession {}

static SESSION: Mutex<Option<DictationSession>> = Mutex::new(None);

pub fn start_dictation_native(app: &AppHandle, locale: &str) -> Result<(), String> {
    let mut session_guard = SESSION.lock().map_err(|e| e.to_string())?;
    if session_guard.is_some() {
        return Err("Dictation already in progress".to_string());
    }

    unsafe {
        let status = SFSpeechRecognizer::authorizationStatus();
        if status == SFSpeechRecognizerAuthorizationStatus::Denied
            || status == SFSpeechRecognizerAuthorizationStatus::Restricted
        {
            return Err("Speech recognition permission denied. Enable in System Settings > Privacy & Security > Speech Recognition.".to_string());
        }

        if status == SFSpeechRecognizerAuthorizationStatus::NotDetermined {
            let (tx, rx) = std::sync::mpsc::channel();
            let block = RcBlock::new(move |auth_status: SFSpeechRecognizerAuthorizationStatus| {
                let _ = tx.send(auth_status);
            });
            SFSpeechRecognizer::requestAuthorization(&block);
            let granted = rx.recv().map_err(|e| e.to_string())?;
            if granted != SFSpeechRecognizerAuthorizationStatus::Authorized {
                return Err("Speech recognition permission not granted".to_string());
            }
        }

        let ns_locale_id = NSString::from_str(locale);
        let ns_locale = NSLocale::initWithLocaleIdentifier(
            NSLocale::alloc(),
            &ns_locale_id,
        );

        let recognizer = SFSpeechRecognizer::initWithLocale(
            SFSpeechRecognizer::alloc(),
            &ns_locale,
        )
        .ok_or("Failed to create speech recognizer for locale")?;

        if !recognizer.isAvailable() {
            return Err(format!("Speech recognizer not available for locale: {}", locale));
        }

        let request = SFSpeechAudioBufferRecognitionRequest::new();
        request.setShouldReportPartialResults(true);

        if recognizer.supportsOnDeviceRecognition() {
            request.setRequiresOnDeviceRecognition(true);
        }

        let engine = AVAudioEngine::new();
        let input_node = engine.inputNode();
        let format = input_node.outputFormatForBus(0);

        let request_for_tap = request.clone();
        let tap_block: RcBlock<dyn Fn(NonNull<AVAudioPCMBuffer>, NonNull<AVAudioTime>)> =
            RcBlock::new(move |buffer: NonNull<AVAudioPCMBuffer>, _time: NonNull<AVAudioTime>| {
                request_for_tap.appendAudioPCMBuffer(buffer.as_ref());
            });

        input_node.installTapOnBus_bufferSize_format_block(
            0,
            1024,
            Some(&format),
            &*tap_block as *const _ as *mut _,
        );

        let app_handle = app.clone();
        let result_block: RcBlock<dyn Fn(*mut SFSpeechRecognitionResult, *mut NSError)> =
            RcBlock::new(
                move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
                    if let Some(err) = error.as_ref() {
                        let desc = err.localizedDescription().to_string();
                        let _ = app_handle.emit("dictation-error", DictationError { message: desc });
                        return;
                    }
                    if let Some(res) = result.as_ref() {
                        let is_final = res.isFinal();
                        let transcription = res.bestTranscription();
                        let text = transcription.formattedString().to_string();
                        let _ = app_handle.emit(
                            "dictation-result",
                            DictationResult { text, is_final },
                        );
                    }
                },
            );

        let task = recognizer.recognitionTaskWithRequest_resultHandler(&request, &result_block);

        engine.startAndReturnError().map_err(|e| e.to_string())?;

        let _ = app.emit("dictation-started", ());

        *session_guard = Some(DictationSession {
            engine,
            request,
            task,
        });
    }

    Ok(())
}

pub fn stop_dictation_native() -> Result<(), String> {
    let mut session_guard = SESSION.lock().map_err(|e| e.to_string())?;
    let session = session_guard.take().ok_or("No dictation session active")?;

    unsafe {
        session.request.endAudio();
        session.task.finish();
        let input_node = session.engine.inputNode();
        input_node.removeTapOnBus(0);
        session.engine.stop();
    }

    Ok(())
}
