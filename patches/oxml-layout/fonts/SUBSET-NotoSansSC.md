# Noto Sans SC F-X058 subset

Source: `ofl/notosanssc/NotoSansSC[wght].ttf` from the Google Fonts `main`
branch, retrieved 2026-08-26.

Source SHA-256:
`a3041811a78c361b1de50f953c805e0244951c21c5bd412f7232ef0d899af0da`

Output SHA-256:
`b06144fa7b2d5212fe21344261449c9350f603e3e2ae625e76306022d024fbe5`

The approved fixture repertoire is ASCII space, comma, digits, Latin letters,
the CJK punctuation `、〈〉`, and `世中你好界`. Reproduce it with FontTools:

```text
pyftsubset NotoSansSC.ttf --output-file=NotoSansSC-FX058-subset.ttf --unicodes=U+0020,U+002C,U+0030-0039,U+0041-005A,U+0061-007A,U+3001,U+3008,U+3009,U+4E16,U+4E2D,U+4F60,U+597D,U+754C --glyph-names --symbol-cmap --legacy-cmap --notdef-glyph --notdef-outline --recommended-glyphs --name-IDs=* --name-legacy --name-languages=* --layout-features=* --no-hinting
```
