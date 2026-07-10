# gpt-image-2 -- brand exploration for Cull
# gpt-image-2 unique presets explore the brand across moods. Add --thinking medium for infographic/diagram subjects; --quality high for finals (~$0.21/img).
# DO use the exact hex codes and fonts below. DON'T add logos, real
#   brand names, or text unless the preset is text-oriented.

scripts/gpt_image_2.py --preset editorial --platform square --quality medium -y \
  "abstract brand mood board for Cull, geometric composition expressing the brand's character, color palette: primary #7aa2f7, accent #bb9af7, text #e0e0e0, background #08080c, success #9ece6a, warning #e0af68, danger #f7768e, muted #7a7fa0, typography: JetBrains Mono, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace, Geist, ui-sans-serif, -apple-system, BlinkMacSystemFont, Helvetica Neue, Arial, sans-serif, EB Garamond, Georgia, Times New Roman, serif, softly rounded corners" \
  cull-editorial.png

scripts/gpt_image_2.py --preset bauhaus --platform square --quality medium -y \
  "abstract brand mood board for Cull, geometric composition expressing the brand's character, color palette: primary #7aa2f7, accent #bb9af7, text #e0e0e0, background #08080c, success #9ece6a, warning #e0af68, danger #f7768e, muted #7a7fa0, typography: JetBrains Mono, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace, Geist, ui-sans-serif, -apple-system, BlinkMacSystemFont, Helvetica Neue, Arial, sans-serif, EB Garamond, Georgia, Times New Roman, serif, softly rounded corners" \
  cull-bauhaus.png

scripts/gpt_image_2.py --preset isometric --platform square --quality medium -y \
  "abstract brand mood board for Cull, geometric composition expressing the brand's character, color palette: primary #7aa2f7, accent #bb9af7, text #e0e0e0, background #08080c, success #9ece6a, warning #e0af68, danger #f7768e, muted #7a7fa0, typography: JetBrains Mono, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace, Geist, ui-sans-serif, -apple-system, BlinkMacSystemFont, Helvetica Neue, Arial, sans-serif, EB Garamond, Georgia, Times New Roman, serif, softly rounded corners" \
  cull-isometric.png

scripts/gpt_image_2.py --preset poster --platform square --quality medium -y \
  "abstract brand mood board for Cull, geometric composition expressing the brand's character, color palette: primary #7aa2f7, accent #bb9af7, text #e0e0e0, background #08080c, success #9ece6a, warning #e0af68, danger #f7768e, muted #7a7fa0, typography: JetBrains Mono, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace, Geist, ui-sans-serif, -apple-system, BlinkMacSystemFont, Helvetica Neue, Arial, sans-serif, EB Garamond, Georgia, Times New Roman, serif, softly rounded corners" \
  cull-poster.png

# nano-banana -- brand exploration for Cull
# nano-banana's edge is accurate in-image TEXT (--model pro) and style anchoring via --reference <img>. Prefer it when the brand name/wordmark must render legibly. Add --dry-run to preview the composed prompt without an API call.
# DO use the exact hex codes and fonts below. DON'T add logos, real
#   brand names, or text unless the preset is text-oriented.

scripts/nano_banana.py --preset editorial --platform square --model pro \
  "abstract brand mood board for Cull, geometric composition expressing the brand's character, color palette: primary #7aa2f7, accent #bb9af7, text #e0e0e0, background #08080c, success #9ece6a, warning #e0af68, danger #f7768e, muted #7a7fa0, typography: JetBrains Mono, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace, Geist, ui-sans-serif, -apple-system, BlinkMacSystemFont, Helvetica Neue, Arial, sans-serif, EB Garamond, Georgia, Times New Roman, serif, softly rounded corners" \
  cull-editorial.png

scripts/nano_banana.py --preset risograph --platform square --model pro \
  "abstract brand mood board for Cull, geometric composition expressing the brand's character, color palette: primary #7aa2f7, accent #bb9af7, text #e0e0e0, background #08080c, success #9ece6a, warning #e0af68, danger #f7768e, muted #7a7fa0, typography: JetBrains Mono, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace, Geist, ui-sans-serif, -apple-system, BlinkMacSystemFont, Helvetica Neue, Arial, sans-serif, EB Garamond, Georgia, Times New Roman, serif, softly rounded corners" \
  cull-risograph.png

scripts/nano_banana.py --preset brutalist --platform square --model pro \
  "abstract brand mood board for Cull, geometric composition expressing the brand's character, color palette: primary #7aa2f7, accent #bb9af7, text #e0e0e0, background #08080c, success #9ece6a, warning #e0af68, danger #f7768e, muted #7a7fa0, typography: JetBrains Mono, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace, Geist, ui-sans-serif, -apple-system, BlinkMacSystemFont, Helvetica Neue, Arial, sans-serif, EB Garamond, Georgia, Times New Roman, serif, softly rounded corners" \
  cull-brutalist.png
