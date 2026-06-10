// First-run onboarding helpers (UX-03 + UX-04).

// Where "set up local AI models" help lives. The app must not bundle or
// auto-download third-party weights (see AGENTS.md), so the affordance is
// a guide, not a download button.
export const MODEL_SETUP_GUIDE_URL = 'https://github.com/glebis/cull/wiki';

// The AI MODELS sidebar section leads with model jargon, so it stays
// collapsed on first run (empty library) and only expands by default once
// the library has content. An explicit user toggle always wins.
export function resolveAiSectionExpanded(userToggle: boolean | null, imageCount: number): boolean {
    if (userToggle !== null) return userToggle;
    return imageCount > 0;
}
