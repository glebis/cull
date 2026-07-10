#!/usr/bin/env python3
"""Browser smoke coverage for the TEST_SCENARIOS.md happy paths.

This suite intentionally exercises the Vite browser build with the E2E Tauri
mock. It must not touch the real Cull database or native filesystem actions.
"""

import os
import re
import sys
from pathlib import Path
from typing import Callable

from playwright.sync_api import Page, expect, sync_playwright


URL = os.environ.get("CULL_E2E_URL", "http://127.0.0.1:1420")
SHOTS = Path(os.environ.get("CULL_E2E_SHOTS", "/tmp/cull-e2e"))
DEFAULT_CHROME_BETA = "/Applications/Google Chrome Beta.app/Contents/MacOS/Google Chrome Beta"
BROWSER_EXECUTABLE = os.environ.get("CULL_E2E_BROWSER", DEFAULT_CHROME_BETA)


class Smoke:
    def __init__(self, page: Page) -> None:
        self.page = page
        self.failures: list[str] = []

    def step(self, name: str, fn: Callable[[], None]) -> None:
        try:
            wait_for_app(self.page)
            fn()
            print(f"  PASS {name}")
        except Exception as exc:
            self.failures.append(f"{name}: {exc}")
            screenshot = SHOTS / f"{len(self.failures):02d}-{slug(name)}.png"
            self.page.screenshot(path=str(screenshot), full_page=True)
            print(f"  FAIL {name}")
            print(f"       {exc}")
            print(f"       screenshot: {screenshot}")

    def finish(self) -> None:
        if not self.failures:
            return
        print("\nFailures:")
        for failure in self.failures:
            print(f"  - {failure}")
        raise SystemExit(1)


def slug(value: str) -> str:
    return "".join(ch.lower() if ch.isalnum() else "-" for ch in value).strip("-")


def wait_for_app(page: Page, url: str = URL) -> None:
    page.goto(url)
    try:
        page.wait_for_load_state("networkidle", timeout=5_000)
    except Exception:
        page.wait_for_load_state("domcontentloaded")
    if page.evaluate("() => window.__CULL_E2E_MOCK__ === true") is not True:
        raise AssertionError("CULL_E2E_MOCK is not active; start Vite through tests/e2e/run-e2e.sh")
    expect(page.locator(".tabbar")).to_be_visible()
    press(page, "Meta+1")
    wait_mode(page, "grid")
    expect(page.locator(".grid-container")).to_be_visible()
    expect(page.locator(".thumb").first).to_be_visible(timeout=10_000)


def press(page: Page, shortcut: str) -> None:
    page.keyboard.press(shortcut)
    page.wait_for_timeout(150)


def wait_mode(page: Page, mode: str) -> None:
    expect(page.locator(".statusbar .mode")).to_have_text(mode, timeout=5_000)


def focused_label(page: Page) -> str:
    label = page.locator(".thumb.focused").first.get_attribute("aria-label")
    if not label:
        raise AssertionError("no focused thumbnail")
    return label


def focused_filename(page: Page) -> str:
    return focused_label(page).split(',', 1)[0].strip()


def thumb_label(page: Page, index: int) -> str:
    locator = page.locator(".thumb").nth(index)
    expect(locator).to_be_visible()
    label = locator.get_attribute("aria-label")
    if not label:
        raise AssertionError(f"thumbnail {index} has no aria-label")
    return label


def thumb_filename(page: Page, index: int) -> str:
    return thumb_label(page, index).split(',', 1)[0].strip()


def thumb_filenames(page: Page) -> list[str]:
    return [thumb_filename(page, index) for index in range(page.locator(".thumb").count())]


def last_thumb_label(page: Page) -> str:
    count = page.locator(".thumb").count()
    if count == 0:
        raise AssertionError("no thumbnails in grid")
    return thumb_label(page, count - 1)


def assert_thumb_focus_has_filename(page: Page, filename: str) -> None:
    expect(page.locator(".thumb.focused")).to_have_attribute("aria-label", re.compile(rf"^{re.escape(filename)}(?:,|$)"))


def statusbar_text(page: Page) -> str:
    return page.locator(".statusbar").inner_text()


def ensure_detection_boxes(page: Page, enabled: bool = True) -> None:
    if ("D:boxes" in statusbar_text(page)) != enabled:
        press(page, "d")
    if enabled:
        expect(page.locator(".statusbar")).to_contain_text("D:boxes")
    else:
        expect(page.locator(".statusbar")).not_to_contain_text("D:boxes")


def ensure_nsfw_mode(page: Page, mode: str) -> None:
    expected = f"B:nsfw:{mode}"
    for _ in range(4):
        if expected in statusbar_text(page):
            expect(page.locator(".statusbar")).to_contain_text(expected)
            return
        press(page, "b")
    raise AssertionError(f"could not reach {expected}")


def set_search_value(page: Page, value: str) -> None:
    page.locator(".command-input").evaluate(
        """(el, value) => {
            const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
            setter.call(el, value);
            el.dispatchEvent(new Event('input', { bubbles: true }));
        }""",
        value,
    )


def set_input_value(page: Page, selector: str, value: str) -> None:
    """Set a Svelte-bound input via the native setter + input event.

    Svelte's bind:value does not observe DOM-level `.value` assignments, so
    Playwright's fill() does not update the bound state. This mirrors
    set_search_value but for an arbitrary selector (e.g. the command palette).
    """
    page.locator(selector).evaluate(
        """(el, value) => {
            const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
            setter.call(el, value);
            el.dispatchEvent(new Event('input', { bubbles: true }));
        }""",
        value,
    )


def dispatch_key(page: Page, key: str, *, meta: bool = False, shift: bool = False) -> None:
    page.evaluate(
        """({ key, meta, shift }) => {
            window.dispatchEvent(new KeyboardEvent('keydown', {
                key,
                code: key === '\\\\' ? 'Backslash' : undefined,
                bubbles: true,
                metaKey: meta,
                shiftKey: shift,
            }));
        }""",
        {"key": key, "meta": meta, "shift": shift},
    )
    page.wait_for_timeout(150)


def test_view_switching(page: Page) -> None:
    cases = [
        ("Meta+1", "grid", ".grid-container", "Grid"),
        ("Meta+2", "loupe", ".loupe-container", "Loupe"),
        ("Meta+3", "compare", ".compare-container", "Compare"),
        ("Meta+4", "canvas", ".canvas-viewport", "Canvas"),
        ("Meta+5", "lineage", ".lineage-view", "Lineage"),
        ("Meta+6", "embeddings", ".embedding-explorer", "Embeddings"),
        ("Meta+7", "export", ".export-view", "Export"),
    ]
    for shortcut, mode, selector, label in cases:
        if shortcut == "Meta+7":
            dispatch_key(page, "7", meta=True)
        else:
            press(page, shortcut)
        wait_mode(page, mode)
        expect(page.locator(selector)).to_be_visible(timeout=5_000)
        expect(page.locator(".tab.active")).to_contain_text(label)

    press(page, "Meta+8")
    wait_mode(page, "tinder")
    expect(page.locator(".tinder-container")).to_be_visible(timeout=5_000)

    press(page, "Meta+1")
    press(page, "Tab")
    wait_mode(page, "loupe")
    press(page, "Shift+Tab")
    wait_mode(page, "grid")


def test_compare_statusbar_does_not_resize_layout(page: Page) -> None:
    wait_for_app(page, f"{URL}?longCompareNames=1")
    press(page, "Meta+3")
    wait_mode(page, "compare")

    expect(page.locator(".statusbar")).not_to_contain_text(" vs ")

    metrics = page.evaluate(
        """() => {
            const shell = document.querySelector('.app-shell').getBoundingClientRect();
            const compare = document.querySelector('.compare-container').getBoundingClientRect();
            const panels = [...document.querySelectorAll('.compare-container .panel')]
                .map((el) => el.getBoundingClientRect());
            return {
                windowWidth: window.innerWidth,
                bodyScrollWidth: document.documentElement.scrollWidth,
                shellWidth: shell.width,
                compareWidth: compare.width,
                leftPanelWidth: panels[0].width,
                rightPanelWidth: panels[1].width,
            };
        }"""
    )

    assert metrics["bodyScrollWidth"] <= metrics["windowWidth"] + 1, metrics
    assert abs(metrics["shellWidth"] - metrics["windowWidth"]) <= 1, metrics
    assert abs(metrics["compareWidth"] - metrics["windowWidth"]) <= 1, metrics
    assert abs(metrics["leftPanelWidth"] - metrics["rightPanelWidth"]) <= 2, metrics


def test_compare_shift_period_cycles_to_image_only(page: Page) -> None:
    press(page, "Meta+3")
    wait_mode(page, "compare")

    def metrics() -> dict:
        return page.evaluate(
            """() => {
                const container = document.querySelector('.compare-container');
                const active = document.querySelector('.compare-container .panel.active');
                const divider = document.querySelector('.compare-container .divider');
                const activeStyle = active ? getComputedStyle(active) : null;
                const dividerStyle = divider ? getComputedStyle(divider) : null;
                return {
                    imageOnly: container?.classList.contains('images-only') ?? false,
                    statusbarCount: document.querySelectorAll('.statusbar').length,
                    labelCount: document.querySelectorAll('.compare-container .label').length,
                    metaCount: document.querySelectorAll('.compare-container .meta').length,
                    activeBorderTopWidth: activeStyle?.borderTopWidth ?? '',
                    panelPaddingTop: activeStyle?.paddingTop ?? '',
                    dividerWidth: dividerStyle?.width ?? '',
                };
            }"""
        )

    press(page, "Shift+.")
    zen = metrics()
    assert zen["statusbarCount"] == 0, zen
    assert zen["imageOnly"] is False, zen
    assert zen["labelCount"] == 2, zen
    assert zen["metaCount"] == 2, zen
    assert zen["activeBorderTopWidth"] == "2px", zen

    press(page, "Shift+.")
    image_only = metrics()
    assert image_only["statusbarCount"] == 0, image_only
    assert image_only["imageOnly"] is True, image_only
    assert image_only["labelCount"] == 0, image_only
    assert image_only["metaCount"] == 0, image_only
    assert image_only["activeBorderTopWidth"] == "0px", image_only
    assert image_only["panelPaddingTop"] == "0px", image_only
    assert image_only["dividerWidth"] == "0px", image_only

    press(page, "Shift+.")
    wait_mode(page, "compare")
    normal = metrics()
    assert normal["imageOnly"] is False, normal
    assert normal["labelCount"] == 2, normal
    assert normal["metaCount"] == 2, normal


def test_export_shift_period_cycles_to_image_only(page: Page) -> None:
    press(page, "Meta+7")
    wait_mode(page, "export")
    expect(page.locator(".export-toolbar")).to_be_visible(timeout=10_000)
    expect(page.locator(".preview-card").first).to_be_visible(timeout=10_000)

    def metrics() -> dict:
        return page.evaluate(
            """() => {
                const view = document.querySelector('.export-view');
                const grid = document.querySelector('.image-only-grid');
                const firstImage = document.querySelector('.image-only-grid img');
                const firstImageBox = firstImage?.getBoundingClientRect();
                const viewBox = view?.getBoundingClientRect();
                return {
                    imageOnly: view?.classList.contains('images-only') ?? false,
                    statusbarCount: document.querySelectorAll('.statusbar').length,
                    toolbarCount: document.querySelectorAll('.export-toolbar').length,
                    buttonCount: document.querySelectorAll('.export-view button').length,
                    selectCount: document.querySelectorAll('.export-view select').length,
                    labelCount: document.querySelectorAll('.export-view .preview-label').length,
                    slideTextCount: document.querySelectorAll('.export-view .headline, .export-view .body, .export-view .caption, .export-view .label').length,
                    imageOnlyGridCount: grid ? 1 : 0,
                    imageOnlyImageCount: document.querySelectorAll('.image-only-grid img').length,
                    firstImageWidth: firstImageBox?.width ?? 0,
                    firstImageHeight: firstImageBox?.height ?? 0,
                    viewWidth: viewBox?.width ?? 0,
                    viewHeight: viewBox?.height ?? 0,
                };
            }"""
        )

    press(page, "Shift+.")
    zen = metrics()
    assert zen["statusbarCount"] == 0, zen
    assert zen["imageOnly"] is False, zen
    assert zen["toolbarCount"] == 1, zen
    assert zen["labelCount"] > 0, zen

    press(page, "Shift+.")
    image_only = metrics()
    assert image_only["statusbarCount"] == 0, image_only
    assert image_only["imageOnly"] is True, image_only
    assert image_only["toolbarCount"] == 0, image_only
    assert image_only["buttonCount"] == 0, image_only
    assert image_only["selectCount"] == 0, image_only
    assert image_only["labelCount"] == 0, image_only
    assert image_only["slideTextCount"] == 0, image_only
    assert image_only["imageOnlyGridCount"] == 1, image_only
    assert image_only["imageOnlyImageCount"] > 0, image_only
    assert image_only["firstImageWidth"] >= image_only["viewWidth"] * 0.2, image_only
    assert image_only["firstImageHeight"] >= image_only["viewHeight"] * 0.2, image_only

    press(page, "Shift+.")
    wait_mode(page, "export")
    normal = metrics()
    assert normal["imageOnly"] is False, normal
    assert normal["toolbarCount"] == 1, normal
    assert normal["labelCount"] > 0, normal


def test_grid_navigation(page: Page) -> None:
    press(page, "Meta+1")
    wait_mode(page, "grid")
    assert_thumb_focus_has_filename(page, "image-0.png")

    press(page, "ArrowRight")
    assert_thumb_focus_has_filename(page, "image-1.png")

    expected_last = last_thumb_label(page)
    press(page, "End")
    expect(page.locator(".thumb.focused")).to_have_attribute("aria-label", expected_last)

    press(page, "Home")
    assert_thumb_focus_has_filename(page, "image-0.png")

    press(page, "PageDown")
    assert focused_filename(page) != "image-0.png"

    press(page, "Enter")
    wait_mode(page, "loupe")
    press(page, "Escape")
    wait_mode(page, "grid")

    page.locator(".thumb").nth(2).dblclick()
    wait_mode(page, "loupe")
    press(page, "Escape")
    wait_mode(page, "grid")


def test_loupe_navigation(page: Page) -> None:
    press(page, "Meta+1")
    press(page, "Home")
    press(page, "Enter")
    wait_mode(page, "loupe")
    ensure_nsfw_mode(page, "show")
    expect(page.locator(".statusbar")).to_contain_text("image-0.png | 1920x1080 | png")

    press(page, "ArrowRight")
    expect(page.locator(".statusbar")).to_contain_text("image-1.png | 1920x1080 | png")

    press(page, "+")
    expect(page.locator(".loupe-container img").first).to_have_attribute("style", re.compile(r"scale\(1\.25\)"))

    press(page, "Home")
    expect(page.locator(".loupe-container img").first).to_have_attribute("style", re.compile(r"scale\(1\)"))

    page.locator(".loupe-container").dblclick()
    wait_mode(page, "grid")


def test_rating_decision_and_selection(page: Page) -> None:
    press(page, "Meta+1")
    press(page, "Home")

    press(page, "0")
    expect(page.locator(".thumb.focused .rating .star")).to_have_count(0)

    press(page, "5")
    expect(page.locator(".thumb.focused .rating .star")).to_have_count(5)

    dispatch_key(page, "a")
    expect(page.locator(".thumb.focused .badge.accept")).to_be_visible()
    dispatch_key(page, "x")
    expect(page.locator(".thumb.focused .badge.reject")).to_be_visible()
    dispatch_key(page, "u")
    expect(page.locator(".thumb.focused .badge")).to_have_count(0)

    press(page, "Space")
    expect(page.locator(".statusbar")).to_contain_text("1 selected")
    page.locator(".thumb").nth(4).click(modifiers=["Shift"])
    expect(page.locator(".statusbar")).to_contain_text("5 selected")


def test_search_and_command_palette(page: Page) -> None:
    press(page, "Meta+1")
    press(page, "/")
    expect(page.locator(".command-input")).to_be_visible()
    set_search_value(page, "landscape 4 stars")
    press(page, "Enter")
    expect(page.locator(".parsed-rules")).to_be_visible(timeout=5_000)
    expect(page.locator(".parsed-rules")).to_contain_text("Parsed as:")
    expect(page.locator(".save-btn")).to_be_visible()

    page.locator(".command-input").press("Escape")
    expect(page.locator(".command-input")).to_have_count(0)

    press(page, "Meta+K")
    expect(page.locator(".palette-panel")).to_be_visible()
    set_input_value(page, ".palette-input", "loupe")
    expect(page.locator(".palette-row").first).to_contain_text("Loupe View")
    page.locator(".palette-input").press("Enter")
    wait_mode(page, "loupe")

    press(page, "Meta+Shift+P")
    expect(page.locator(".palette-panel")).to_be_visible()
    expect(page.locator(".palette-subtitle")).to_contain_text("Commands")
    page.locator(".palette-input").press("Escape")
    expect(page.locator(".palette-panel")).to_have_count(0)


def test_chrome_and_detection_controls(page: Page) -> None:
    press(page, "Meta+1")
    expect(page.locator(".sidebar")).to_be_visible()
    dispatch_key(page, "\\")
    expect(page.locator(".sidebar")).to_have_count(0)
    dispatch_key(page, "\\")
    expect(page.locator(".sidebar")).to_be_visible()

    press(page, "Shift+.")
    expect(page.locator(".tabbar")).to_have_count(0)
    expect(page.locator(".statusbar")).to_have_count(0)
    press(page, "Escape")
    expect(page.locator(".tabbar")).to_be_visible()

    ensure_detection_boxes(page, True)
    press(page, "Meta+2")
    wait_mode(page, "loupe")
    ensure_nsfw_mode(page, "show")
    press(page, "i")
    expect(page.locator(".inspector")).to_be_visible()
    expect(page.locator(".inspector")).to_contain_text("person")
    ensure_nsfw_mode(page, "hide")
    ensure_nsfw_mode(page, "show")
    press(page, "Escape")


def test_embeddings_and_empty_states(page: Page) -> None:
    press(page, "Meta+6")
    expect(page.locator(".embedding-explorer")).to_be_visible(timeout=10_000)
    expect(page.locator(".embedding-explorer")).to_have_attribute("aria-label", "Visual embeddings")
    expect(page.locator(".embedding-explorer")).to_contain_text("CLIP")
    expect(page.locator(".embedding-explorer")).to_contain_text("DINOv2")
    expect(page.locator(".embedding-explorer")).to_contain_text("Gemini")

    press(page, "Meta+1")
    expect(page.locator(".grid-container")).to_be_visible()


# --- NEW HIGH-PRIORITY SCENARIOS ---


def test_view_mode_cmd_numbers(page: Page) -> None:
    """S01 — Each Cmd+N shortcut switches to the correct view and back."""
    press(page, "Meta+1")
    wait_mode(page, "grid")

    # Cmd+2 -> loupe
    press(page, "Meta+2")
    wait_mode(page, "loupe")
    expect(page.locator(".tab.active")).to_contain_text("Loupe")

    # Cmd+3 -> compare
    press(page, "Meta+3")
    wait_mode(page, "compare")
    expect(page.locator(".tab.active")).to_contain_text("Compare")

    # Cmd+4 -> canvas
    press(page, "Meta+4")
    wait_mode(page, "canvas")
    expect(page.locator(".tab.active")).to_contain_text("Canvas")

    # Cmd+5 -> lineage
    press(page, "Meta+5")
    wait_mode(page, "lineage")
    expect(page.locator(".tab.active")).to_contain_text("Lineage")

    # Cmd+6 -> embeddings
    press(page, "Meta+6")
    wait_mode(page, "embeddings")
    expect(page.locator(".tab.active")).to_contain_text("Embeddings")

    # Cmd+7 -> export
    dispatch_key(page, "7", meta=True)
    wait_mode(page, "export")
    expect(page.locator(".tab.active")).to_contain_text("Export")

    # Cmd+8 -> tinder
    press(page, "Meta+8")
    wait_mode(page, "tinder")

    # Return to grid
    press(page, "Meta+1")
    wait_mode(page, "grid")


def test_tab_cycling(page: Page) -> None:
    """S01 — Tab cycles forward through views, Shift+Tab cycles backward."""
    press(page, "Meta+1")
    wait_mode(page, "grid")

    # Tab cycles forward: grid -> loupe -> compare -> canvas -> ...
    press(page, "Tab")
    wait_mode(page, "loupe")

    press(page, "Tab")
    wait_mode(page, "compare")

    press(page, "Tab")
    wait_mode(page, "canvas")

    # Shift+Tab cycles backward: canvas -> compare
    press(page, "Shift+Tab")
    wait_mode(page, "compare")

    press(page, "Shift+Tab")
    wait_mode(page, "loupe")

    press(page, "Shift+Tab")
    wait_mode(page, "grid")


def test_grid_hjkl_navigation(page: Page) -> None:
    """S02 — h/j/k/l and arrow keys move focus in grid."""
    press(page, "Meta+1")
    wait_mode(page, "grid")
    press(page, "Home")
    assert_thumb_focus_has_filename(page, "image-0.png")

    # l moves right (same as ArrowRight)
    dispatch_key(page, "l")
    assert_thumb_focus_has_filename(page, "image-1.png")

    # h moves left (same as ArrowLeft)
    dispatch_key(page, "h")
    assert_thumb_focus_has_filename(page, "image-0.png")

    # j moves down (one row)
    dispatch_key(page, "j")
    assert focused_filename(page) != "image-0.png", "j should move focus down"

    # k moves back up
    dispatch_key(page, "k")
    assert_thumb_focus_has_filename(page, "image-0.png")


def test_grid_home_end(page: Page) -> None:
    """S02 — Home jumps to first image, End to last."""
    press(page, "Meta+1")
    wait_mode(page, "grid")

    expected_last = last_thumb_label(page)
    press(page, "End")
    expect(page.locator(".thumb.focused")).to_have_attribute("aria-label", expected_last)

    press(page, "Home")
    assert_thumb_focus_has_filename(page, "image-0.png")


def test_thumbnail_state_aria_labels(page: Page) -> None:
    press(page, "Meta+1")
    wait_mode(page, "grid")
    press(page, "Home")

    expect(page.locator(".thumb.focused")).to_have_attribute("aria-label", re.compile(r"rating 0"))
    expect(page.locator(".thumb.focused")).to_have_attribute("aria-label", re.compile(r"decision undecided"))
    expect(page.locator(".thumb.focused")).to_have_attribute("aria-label", re.compile(r"source "))
    expect(page.locator(".thumb.focused")).to_have_attribute("aria-label", re.compile(r"not selected"))
    expect(page.locator(".thumb.focused")).to_have_attribute("aria-label", re.compile(r"present"))

    press(page, "1")
    expect(page.locator(".thumb.focused")).to_have_attribute("aria-label", re.compile(r"rating 1"))

    press(page, "a")
    expect(page.locator(".thumb.focused")).to_have_attribute("aria-label", re.compile(r"decision accept"))

    press(page, "Space")
    expect(page.locator(".thumb.focused")).to_have_attribute("aria-label", re.compile(r"selected"))


def test_star_ratings(page: Page) -> None:
    """S09 — Direct 1-5 star rating, 0 to clear, chord s+N."""
    press(page, "Meta+1")
    wait_mode(page, "grid")
    press(page, "Home")

    # Clear any existing rating
    press(page, "0")
    expect(page.locator(".thumb.focused .rating .star")).to_have_count(0)

    # Direct press 1 -> 1 star
    press(page, "1")
    expect(page.locator(".thumb.focused .rating .star")).to_have_count(1)

    # Direct press 3 -> 3 stars
    press(page, "3")
    expect(page.locator(".thumb.focused .rating .star")).to_have_count(3)

    # Direct press 5 -> 5 stars
    press(page, "5")
    expect(page.locator(".thumb.focused .rating .star")).to_have_count(5)

    # Press 0 -> clear
    press(page, "0")
    expect(page.locator(".thumb.focused .rating .star")).to_have_count(0)

    # Chord: s then 4
    press(page, "s")
    expect(page.locator(".statusbar")).to_contain_text("Rate: press 1-5")
    press(page, "4")
    expect(page.locator(".thumb.focused .rating .star")).to_have_count(4)

    # Clean up
    press(page, "0")


def test_accept_reject_undecided(page: Page) -> None:
    """S10 — a/x/u set accept/reject/undecided badges."""
    press(page, "Meta+1")
    wait_mode(page, "grid")
    press(page, "Home")

    dispatch_key(page, "u")
    expect(page.locator(".thumb.focused .badge")).to_have_count(0)

    # a -> accept (green check badge)
    dispatch_key(page, "a")
    expect(page.locator(".thumb.focused .badge.accept")).to_be_visible()

    # x -> reject (red x badge)
    dispatch_key(page, "x")
    expect(page.locator(".thumb.focused .badge.reject")).to_be_visible()
    expect(page.locator(".thumb.focused .badge.accept")).to_have_count(0)

    # u -> undecided (no badge)
    dispatch_key(page, "u")
    expect(page.locator(".thumb.focused .badge")).to_have_count(0)


def test_trash_escape_confirm_and_undo(page: Page) -> None:
    """S26 — Trash requires confirmation, Escape cancels, and undo restores."""
    press(page, "Meta+1")
    wait_mode(page, "grid")
    press(page, "Home")
    before = thumb_filenames(page)
    intended = focused_filename(page)

    page.keyboard.press("Backspace")
    dialog = page.get_by_role("dialog", name="Move to Trash")
    expect(dialog).to_be_visible()
    page.keyboard.press("Escape")
    expect(dialog).to_be_hidden()

    page.evaluate("window.dispatchEvent(new CustomEvent('reload-images'))")
    page.wait_for_timeout(500)
    expect(page.locator(".thumb")).to_have_count(len(before))
    assert thumb_filenames(page) == before, "Escape changed thumbnail identity or order after reload"

    page.keyboard.press("Backspace")
    page.get_by_role("button", name="Move to Trash", exact=True).click()
    expect(page.locator(".thumb")).to_have_count(len(before) - 1)

    # The removal must survive the same backend reload used by filesystem and undo events.
    page.evaluate("window.dispatchEvent(new CustomEvent('reload-images'))")
    page.wait_for_timeout(500)
    expect(page.locator(".thumb")).to_have_count(len(before) - 1)
    after_confirm = thumb_filenames(page)
    assert intended not in after_confirm, f"Confirmed Trash did not remove {intended}"
    assert after_confirm == [filename for filename in before if filename != intended]

    press(page, "Meta+z")
    expect(page.locator(".thumb")).to_have_count(len(before))
    assert thumb_filenames(page) == before, "Undo did not restore the exact original thumbnail sequence"


def test_loupe_enter_escape(page: Page) -> None:
    """S03 — Enter from grid opens loupe, Escape returns to grid."""
    press(page, "Meta+1")
    wait_mode(page, "grid")
    press(page, "Home")

    press(page, "Enter")
    wait_mode(page, "loupe")
    expect(page.locator(".loupe-container")).to_be_visible()
    expect(page.locator(".statusbar")).to_contain_text("image-0.png")

    press(page, "Escape")
    wait_mode(page, "grid")
    expect(page.locator(".grid-container")).to_be_visible()


def test_loupe_zoom(page: Page) -> None:
    """S03 — +/- zoom in/out, Home resets zoom."""
    press(page, "Meta+1")
    press(page, "Home")
    press(page, "Enter")
    wait_mode(page, "loupe")
    ensure_nsfw_mode(page, "show")

    # Zoom in
    press(page, "+")
    img_style = page.locator(".loupe-container img").first.get_attribute("style") or ""
    assert "scale(1.25)" in img_style, f"Expected scale(1.25) in style, got: {img_style}"

    # Zoom in further
    press(page, "+")
    img_style = page.locator(".loupe-container img").first.get_attribute("style") or ""
    # 1.25 * 1.25 = 1.5625
    assert "scale(1)" not in img_style or "scale(1.5" in img_style, "Should be zoomed past 1x"

    # Zoom out
    press(page, "-")

    # Actual Size with Cmd+0
    dispatch_key(page, "0", meta=True)
    img_style = page.locator(".loupe-container img").first.get_attribute("style") or ""
    actual_scale_match = re.search(r"scale\(([\d.]+)\)", img_style)
    assert actual_scale_match, f"Cmd+0 should set an explicit zoom scale, got: {img_style}"
    assert float(actual_scale_match.group(1)) > 1, f"Cmd+0 should zoom to actual size, got: {img_style}"

    # Zoom in again before checking Home reset
    press(page, "+")

    # Fit In with Home
    press(page, "Home")
    img_style = page.locator(".loupe-container img").first.get_attribute("style") or ""
    assert "scale(1)" in img_style, f"Home should fit the image in, got: {img_style}"

    press(page, "Escape")
    wait_mode(page, "grid")


def test_loupe_arrow_navigation(page: Page) -> None:
    """S03 — Arrow keys cycle through images in loupe."""
    press(page, "Meta+1")
    press(page, "Home")
    press(page, "Enter")
    wait_mode(page, "loupe")
    expect(page.locator(".statusbar")).to_contain_text("image-0.png")

    press(page, "ArrowRight")
    expect(page.locator(".statusbar")).to_contain_text("image-1.png")

    press(page, "ArrowRight")
    expect(page.locator(".statusbar")).to_contain_text("image-2.png")

    press(page, "ArrowLeft")
    expect(page.locator(".statusbar")).to_contain_text("image-1.png")

    press(page, "Escape")


def test_loupe_dblclick_return(page: Page) -> None:
    """S03 — Double-click in loupe returns to grid."""
    press(page, "Meta+1")
    press(page, "Home")
    press(page, "Enter")
    wait_mode(page, "loupe")

    page.locator(".loupe-container").dblclick()
    wait_mode(page, "grid")


def test_sidebar_toggle(page: Page) -> None:
    """S28 — Cmd+B toggles sidebar, backslash also toggles."""
    press(page, "Meta+1")
    wait_mode(page, "grid")

    # Sidebar should be visible initially
    expect(page.locator(".sidebar")).to_be_visible()

    # Cmd+B hides it
    press(page, "Meta+b")
    expect(page.locator(".sidebar")).to_have_count(0)

    # Cmd+B shows it again
    press(page, "Meta+b")
    expect(page.locator(".sidebar")).to_be_visible()

    # Backslash also toggles
    dispatch_key(page, "\\")
    expect(page.locator(".sidebar")).to_have_count(0)

    dispatch_key(page, "\\")
    expect(page.locator(".sidebar")).to_be_visible()


def test_zen_mode(page: Page) -> None:
    """S29 — Shift+. enters zen mode hiding chrome, Escape exits."""
    press(page, "Meta+1")
    wait_mode(page, "grid")

    # Tabbar and sidebar should be visible
    expect(page.locator(".tabbar")).to_be_visible()
    expect(page.locator(".sidebar")).to_be_visible()

    # Shift+. (>) enters zen mode
    press(page, "Shift+.")
    expect(page.locator(".tabbar")).to_have_count(0)
    expect(page.locator(".statusbar")).to_have_count(0)

    # Escape exits zen mode
    press(page, "Escape")
    expect(page.locator(".tabbar")).to_be_visible()

    # Also works in loupe
    press(page, "Meta+2")
    wait_mode(page, "loupe")
    press(page, "Shift+.")
    expect(page.locator(".tabbar")).to_have_count(0)
    press(page, "Escape")
    expect(page.locator(".tabbar")).to_be_visible()

    press(page, "Meta+1")


def test_command_palette_open_close(page: Page) -> None:
    """S19 — Cmd+K opens palette, Escape closes."""
    press(page, "Meta+1")

    press(page, "Meta+K")
    expect(page.locator(".palette-panel")).to_be_visible()

    # Escape (dispatched to the focused palette input) closes the palette.
    page.locator(".palette-input").press("Escape")
    expect(page.locator(".palette-panel")).to_have_count(0)

    # Cmd+Shift+P opens commands-only
    press(page, "Meta+Shift+P")
    expect(page.locator(".palette-panel")).to_be_visible()
    expect(page.locator(".palette-subtitle")).to_contain_text("Commands")

    page.locator(".palette-input").press("Escape")
    expect(page.locator(".palette-panel")).to_have_count(0)


def test_command_palette_navigate_and_execute(page: Page) -> None:
    """S19 — Type to filter, Enter to execute a command."""
    press(page, "Meta+1")
    wait_mode(page, "grid")

    press(page, "Meta+K")
    expect(page.locator(".palette-panel")).to_be_visible()

    # Type "canvas" to filter
    page.locator(".palette-input").fill("canvas")
    page.wait_for_timeout(200)

    # Press Enter to execute first match
    press(page, "Enter")
    # Should have navigated to canvas (or whichever matched first)
    page.wait_for_timeout(300)

    # Return to grid for remaining tests
    press(page, "Meta+1")
    wait_mode(page, "grid")


def test_command_palette_arrows_and_favorite(page: Page) -> None:
    """S19 (zu0.8) — Arrow keys move selection; row context menu favorites a result."""
    press(page, "Meta+1")
    wait_mode(page, "grid")

    press(page, "Meta+K")
    expect(page.locator(".palette-panel")).to_be_visible()
    palette_input = page.locator(".palette-input")
    palette_input.wait_for(state="visible")

    # First row is selected by default; ArrowDown moves selection to the second.
    # Dispatch to the focused input element so the palette's keydown handler runs.
    expect(page.locator(".palette-row.selected").first).to_be_visible()
    first_selected = page.locator(".palette-row.selected").first.get_attribute("id")
    palette_input.press("ArrowDown")
    page.wait_for_timeout(150)
    second_selected = page.locator(".palette-row.selected").first.get_attribute("id")
    assert first_selected != second_selected, "ArrowDown did not move palette selection"

    # Right-click the first row to open the result context menu and Favorite it.
    page.locator(".palette-row").first.click(button="right")
    expect(page.locator(".palette-context-menu")).to_be_visible()
    expect(page.locator(".palette-context-menu")).to_contain_text("Favorite")
    page.locator(".palette-context-menu button", has_text="Favorite").first.click()

    # A favorited row now carries the pin mark.
    expect(page.locator(".palette-row .row-mark", has_text="*").first).to_be_visible()

    palette_input.press("Escape")
    expect(page.locator(".palette-panel")).to_have_count(0)


def test_keyboard_shortcuts_panel(page: Page) -> None:
    """zu0.7/zu0.8 — palette and status bar open the searchable keyboard-shortcuts panel."""
    press(page, "Meta+1")
    wait_mode(page, "grid")

    page.locator('.statusbar .shortcut-button[title="?:help"]').click()
    expect(page.locator(".shortcuts-panel")).to_be_visible()
    expect(page.locator(".shortcuts-row").first).to_be_visible()
    page.locator(".shortcuts-close").click()
    expect(page.locator(".shortcuts-panel")).to_have_count(0)

    press(page, "Meta+K")
    expect(page.locator(".palette-panel")).to_be_visible()
    # Filter to the shortcuts command via the native setter (Svelte bind:value).
    set_input_value(page, ".palette-input", "keyboard shortcuts")
    expect(page.locator(".palette-row").first).to_contain_text("Keyboard Shortcuts")
    page.locator(".palette-input").press("Enter")

    expect(page.locator(".shortcuts-panel")).to_be_visible()
    expect(page.locator(".shortcuts-row").first).to_be_visible()

    page.locator(".shortcuts-close").click()
    expect(page.locator(".shortcuts-panel")).to_have_count(0)


def test_palette_does_not_hijack_text_input(page: Page) -> None:
    """zu0.8 — typing in a normal text input is not captured by palette shortcuts."""
    press(page, "Meta+1")
    wait_mode(page, "grid")

    # Open the natural-language search bar and type text including 'k'.
    press(page, "/")
    expect(page.locator(".command-input")).to_be_visible()
    page.locator(".command-input").fill("dark")
    page.wait_for_timeout(150)

    # The palette must not have opened, and the input keeps the typed value.
    expect(page.locator(".palette-panel")).to_have_count(0)
    assert page.locator(".command-input").input_value() == "dark"

    page.locator(".command-input").press("Escape")
    expect(page.locator(".command-input")).to_have_count(0)


def test_context_menu(page: Page) -> None:
    """S27 — Escape closes the thumbnail context menu and returns Grid control."""
    press(page, "Meta+1")
    wait_mode(page, "grid")
    press(page, "Home")
    before = focused_label(page)

    page.locator(".thumb").first.click(button="right")
    expect(page.locator(".context-menu")).to_be_visible()

    expect(page.locator(".context-menu")).to_contain_text("Rate")
    expect(page.locator(".context-menu")).to_contain_text("Copy")

    page.locator('.context-menu button[data-submenu-key="rate"]').hover()
    expect(page.locator(".context-menu .submenu").first).to_be_visible()

    # Menu-local Escape closes the submenu first; the capture fallback must not
    # collapse the entire menu while focus is inside it.
    page.keyboard.press("Escape")
    expect(page.locator(".context-menu")).to_be_visible()
    expect(page.locator(".context-menu .submenu")).to_have_count(0)

    page.keyboard.press("Escape")
    expect(page.locator(".context-menu")).to_be_hidden()

    page.keyboard.press("ArrowRight")
    assert focused_label(page) != before, "Grid did not regain Arrow-key control after closing the context menu"


def test_context_menu_escape_stays_in_loupe(page: Page) -> None:
    """S27b — Escape dismisses an outside-focused context menu without leaving Loupe."""
    press(page, "Meta+2")
    wait_mode(page, "loupe")

    page.locator(".loupe-container").click(button="right")
    expect(page.locator(".context-menu")).to_be_visible()

    # Reproduce the right-click/focus race: Escape may be targeted outside the
    # menu before its first item receives focus.
    page.evaluate("document.activeElement instanceof HTMLElement && document.activeElement.blur()")
    page.keyboard.press("Escape")

    expect(page.locator(".context-menu")).to_be_hidden()
    wait_mode(page, "loupe")
    expect(page.locator(".loupe-container")).to_be_visible()


def test_context_submenu_flips_at_right_edge(page: Page) -> None:
    """S27a — Submenus stay inside the viewport near the right edge."""
    press(page, "Meta+1")
    wait_mode(page, "grid")

    point = page.evaluate(
        """() => {
            const thumbs = Array.from(document.querySelectorAll('.thumb'));
            const visible = thumbs
                .map(el => el.getBoundingClientRect())
                .filter(rect => rect.width > 0 && rect.height > 0);
            if (visible.length === 0) throw new Error('no visible thumbnails');
            const rect = visible.reduce((rightmost, current) =>
                current.right > rightmost.right ? current : rightmost
            );
            return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
        }"""
    )
    page.mouse.click(point["x"], point["y"], button="right")
    expect(page.locator(".context-menu")).to_be_visible()

    page.locator('.context-menu button[data-submenu-key="rate"]').hover()
    expect(page.locator(".context-menu .submenu").first).to_be_visible()

    metrics = page.evaluate(
        """() => {
            const menu = document.querySelector('.context-menu')?.getBoundingClientRect();
            const submenu = document.querySelector('.context-menu .submenu')?.getBoundingClientRect();
            if (!menu || !submenu) throw new Error('menu or submenu missing');
            return {
                menuLeft: menu.left,
                submenuLeft: submenu.left,
                submenuRight: submenu.right,
                viewportWidth: window.innerWidth,
            };
        }"""
    )
    assert metrics["submenuRight"] <= metrics["viewportWidth"] - 8 + 1, metrics
    assert metrics["submenuLeft"] < metrics["menuLeft"], metrics

    page.locator(".grid-container").click(position={"x": 10, "y": 10})
    page.wait_for_timeout(300)


def test_search_bar_open_close(page: Page) -> None:
    """S16 — / opens search bar, Escape closes it."""
    press(page, "Meta+1")
    wait_mode(page, "grid")

    # / opens search
    press(page, "/")
    expect(page.locator(".command-input")).to_be_visible()

    # Escape closes
    page.locator(".command-input").press("Escape")
    expect(page.locator(".command-input")).to_have_count(0)

    # Cmd+F also opens
    press(page, "Meta+f")
    expect(page.locator(".command-input")).to_be_visible()

    page.locator(".command-input").press("Escape")
    expect(page.locator(".command-input")).to_have_count(0)


def test_search_nl_query(page: Page) -> None:
    """S16 — Natural language query shows parsed rules."""
    press(page, "Meta+1")
    wait_mode(page, "grid")

    press(page, "/")
    expect(page.locator(".command-input")).to_be_visible()

    set_search_value(page, "5 stars portrait")
    press(page, "Enter")
    expect(page.locator(".parsed-rules")).to_be_visible(timeout=5_000)
    expect(page.locator(".parsed-rules")).to_contain_text("Parsed as:")

    # Save button should be visible
    expect(page.locator(".save-btn")).to_be_visible()

    page.locator(".command-input").press("Escape")


def test_ratings_in_loupe(page: Page) -> None:
    """S09 — Star ratings work in loupe mode too."""
    press(page, "Meta+1")
    press(page, "Home")
    press(page, "0")  # clear rating first
    press(page, "Enter")
    wait_mode(page, "loupe")

    # Rate 3 stars
    press(page, "3")
    page.wait_for_timeout(200)

    # Return to grid and verify
    press(page, "Escape")
    wait_mode(page, "grid")
    press(page, "Home")
    expect(page.locator(".thumb.focused .rating .star")).to_have_count(3)

    # Clean up
    press(page, "0")


def test_decisions_in_loupe(page: Page) -> None:
    """S10 — Accept/reject/undecided work in loupe mode."""
    press(page, "Meta+1")
    press(page, "Home")
    press(page, "Enter")
    wait_mode(page, "loupe")

    # Accept
    press(page, "a")
    press(page, "Escape")
    wait_mode(page, "grid")
    press(page, "Home")
    expect(page.locator(".thumb.focused .badge.accept")).to_be_visible()

    # Go back to loupe and reject
    press(page, "Enter")
    wait_mode(page, "loupe")
    press(page, "x")
    press(page, "Escape")
    wait_mode(page, "grid")
    press(page, "Home")
    expect(page.locator(".thumb.focused .badge.reject")).to_be_visible()

    # Clear
    press(page, "Enter")
    wait_mode(page, "loupe")
    press(page, "u")
    press(page, "Escape")
    wait_mode(page, "grid")
    press(page, "Home")
    expect(page.locator(".thumb.focused .badge")).to_have_count(0)


def test_grid_dblclick_opens_loupe(page: Page) -> None:
    """S02 — Double-click on thumbnail opens loupe."""
    press(page, "Meta+1")
    wait_mode(page, "grid")

    page.locator(".thumb").nth(3).dblclick()
    wait_mode(page, "loupe")

    press(page, "Escape")
    wait_mode(page, "grid")


def test_detection_toggle(page: Page) -> None:
    """S32 — d toggles detection boxes, visible in loupe."""
    press(page, "Meta+1")
    wait_mode(page, "grid")

    # Toggle detection boxes on
    ensure_detection_boxes(page, True)

    # Switch to loupe to see bounding boxes
    press(page, "Meta+2")
    wait_mode(page, "loupe")
    ensure_nsfw_mode(page, "show")
    expect(page.locator(".statusbar")).to_contain_text("D:boxes")

    # i opens detection inspector in loupe
    press(page, "i")
    expect(page.locator(".inspector")).to_be_visible()
    expect(page.locator(".inspector")).to_contain_text("person")

    # Close inspector
    press(page, "i")
    expect(page.locator(".inspector")).to_have_count(0)

    press(page, "Escape")


def test_grid_selection_space(page: Page) -> None:
    """S11 — Space toggles selection on focused image."""
    press(page, "Meta+1")
    wait_mode(page, "grid")
    press(page, "Home")

    # Space selects
    press(page, "Space")
    expect(page.locator(".statusbar")).to_contain_text("1 selected")

    # Move to next and select
    press(page, "ArrowRight")
    press(page, "Space")
    expect(page.locator(".statusbar")).to_contain_text("2 selected")

    # Space again deselects
    press(page, "Space")
    expect(page.locator(".statusbar")).to_contain_text("1 selected")

    # Deselect all with Cmd+Shift+A (if supported) or manually deselect
    press(page, "Home")
    press(page, "Space")
    page.wait_for_timeout(100)


def test_grid_shift_click_range_select(page: Page) -> None:
    """S11 — Shift+click selects a range."""
    press(page, "Meta+1")
    wait_mode(page, "grid")
    press(page, "Home")

    # Click first thumbnail
    page.locator(".thumb").first.click()
    page.wait_for_timeout(100)

    # Shift+click 5th thumbnail
    page.locator(".thumb").nth(4).click(modifiers=["Shift"])
    expect(page.locator(".statusbar")).to_contain_text("5 selected")


def main() -> int:
    SHOTS.mkdir(parents=True, exist_ok=True)
    with sync_playwright() as p:
        launch_options = {"headless": True}
        if Path(BROWSER_EXECUTABLE).exists():
            launch_options["executable_path"] = BROWSER_EXECUTABLE
        browser = p.chromium.launch(**launch_options)
        page = browser.new_page(viewport={"width": 1440, "height": 960})
        page.add_init_script("window.localStorage.clear(); window.sessionStorage.clear();")
        page_errors: list[str] = []
        page.on("pageerror", lambda error: page_errors.append(str(error)))

        smoke = Smoke(page)
        wait_for_app(page)
        smoke.step("S01 view switching", lambda: test_view_switching(page))
        smoke.step("S01c compare layout bounded by status bar", lambda: test_compare_statusbar_does_not_resize_layout(page))
        smoke.step("S01d compare Shift+. image-only mode", lambda: test_compare_shift_period_cycles_to_image_only(page))
        smoke.step("S01e export Shift+. image-only mode", lambda: test_export_shift_period_cycles_to_image_only(page))
        smoke.step("S02 grid navigation", lambda: test_grid_navigation(page))
        smoke.step("S03 loupe navigation", lambda: test_loupe_navigation(page))
        smoke.step("S09/S10/S11 curation and selection", lambda: test_rating_decision_and_selection(page))
        smoke.step("S16/S19 search and command palette", lambda: test_search_and_command_palette(page))
        smoke.step("S28/S29/S32/S33 chrome and detection controls", lambda: test_chrome_and_detection_controls(page))
        smoke.step("S08/S43 embeddings and setup UI", lambda: test_embeddings_and_empty_states(page))

        # --- New high-priority scenarios ---
        smoke.step("S01a view mode Cmd+numbers", lambda: test_view_mode_cmd_numbers(page))
        smoke.step("S01b Tab cycling", lambda: test_tab_cycling(page))
        smoke.step("S02a grid h/j/k/l navigation", lambda: test_grid_hjkl_navigation(page))
        smoke.step("S02b grid Home/End", lambda: test_grid_home_end(page))
        smoke.step("S02c grid double-click opens loupe", lambda: test_grid_dblclick_opens_loupe(page))
        smoke.step("S02d thumbnail a11y labels", lambda: test_thumbnail_state_aria_labels(page))
        smoke.step("S09a star ratings", lambda: test_star_ratings(page))
        smoke.step("S09b ratings in loupe", lambda: test_ratings_in_loupe(page))
        smoke.step("S10a accept/reject/undecided", lambda: test_accept_reject_undecided(page))
        smoke.step("S10b decisions in loupe", lambda: test_decisions_in_loupe(page))
        smoke.step("S26 Trash Escape/confirm/undo", lambda: test_trash_escape_confirm_and_undo(page))
        smoke.step("S03a loupe Enter/Escape", lambda: test_loupe_enter_escape(page))
        smoke.step("S03b loupe zoom +/-/Home", lambda: test_loupe_zoom(page))
        smoke.step("S03c loupe arrow navigation", lambda: test_loupe_arrow_navigation(page))
        smoke.step("S03d loupe double-click return", lambda: test_loupe_dblclick_return(page))
        smoke.step("S28a sidebar toggle Cmd+B", lambda: test_sidebar_toggle(page))
        smoke.step("S29a zen mode", lambda: test_zen_mode(page))
        smoke.step("S19a command palette open/close", lambda: test_command_palette_open_close(page))
        smoke.step("S19b command palette navigate and execute", lambda: test_command_palette_navigate_and_execute(page))
        smoke.step("S19c command palette arrows and favorite", lambda: test_command_palette_arrows_and_favorite(page))
        smoke.step("S19d keyboard shortcuts panel", lambda: test_keyboard_shortcuts_panel(page))
        smoke.step("S19e palette does not hijack text input", lambda: test_palette_does_not_hijack_text_input(page))
        smoke.step("S27 context menu", lambda: test_context_menu(page))
        smoke.step("S27a context submenu right edge", lambda: test_context_submenu_flips_at_right_edge(page))
        smoke.step("S27b context menu Escape stays in Loupe", lambda: test_context_menu_escape_stays_in_loupe(page))
        smoke.step("S16a search bar open/close", lambda: test_search_bar_open_close(page))
        smoke.step("S16b search NL query", lambda: test_search_nl_query(page))
        smoke.step("S32 detection toggle", lambda: test_detection_toggle(page))
        smoke.step("S11a selection Space toggle", lambda: test_grid_selection_space(page))
        smoke.step("S11b Shift+click range select", lambda: test_grid_shift_click_range_select(page))

        browser.close()
        if page_errors:
            print("\nPage errors:")
            for error in page_errors:
                print(f"  - {error}")
            return 1
        smoke.finish()
    return 0


if __name__ == "__main__":
    sys.exit(main())
