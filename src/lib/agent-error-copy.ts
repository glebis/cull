export interface AgentFailureCopy {
    title: string;
    detail: string;
}

function errorText(error: unknown): string {
    if (error instanceof Error) return error.message;
    if (typeof error === 'string') return error;
    return String(error ?? '');
}

export function agentFailureCopy(error: unknown): AgentFailureCopy {
    const normalized = errorText(error).toLowerCase();

    if (/(?:api key|credential).*(?:missing|not (?:set|configured|found|available|provided))|(?:missing|no).*(?:api key|credential)/.test(normalized)) {
        return {
            title: 'AI access is not configured',
            detail: 'Sign in to Claude Code or configure the required provider, then try again.',
        };
    }

    if (/(?:budget|spending limit|cost limit).*(?:exceed|reached|limit)|max(?:imum)?[_ ]?budget/.test(normalized)) {
        return {
            title: 'Agent budget reached',
            detail: 'Reduce the selection or shorten the instruction, then try again.',
        };
    }

    if (/(?:timed? out|timeout|deadline exceeded)/.test(normalized)) {
        return {
            title: 'Agent request timed out',
            detail: 'Try again with fewer images or a shorter instruction.',
        };
    }

    if (/(?:unauthori[sz]ed|forbidden|authentication|auth failed|invalid (?:api )?key|\b40[13]\b)/.test(normalized)) {
        return {
            title: 'Claude authentication failed',
            detail: 'Sign in to Claude Code again, then retry the request.',
        };
    }

    return {
        title: 'Agent request failed',
        detail: 'Try again. If the problem continues, check Agent Access Settings.',
    };
}

export function agentActivityMessage(event: { message: string; is_error: boolean }): string {
    return event.is_error ? agentFailureCopy(event.message).detail : event.message;
}

export function agentActivityPhase(phase: string, isError: boolean): string {
    if (isError) return 'Error';
    const label = phase.replace(/^sdk_/, '').replaceAll('_', ' ').trim();
    return label ? label[0].toUpperCase() + label.slice(1) : 'Activity';
}
