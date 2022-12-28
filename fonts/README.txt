Tools to manage these
   fc-query        shows range of font, and other details
   font-manager    to see all the glyphs at all the unicode codepoints
   pyftsubset      to suck a subset of a font out into your own font file

      pyftsubset INPUT.ttf  --unicodes=U+0400-045F,U+0490-0491,U+04B0-04B1,U+2116 \
                 --output-file=OUTPUT.woff2
                 --flavor=woff2

      pyftmerge input1.ttf input2.ttf > output.ttf


    NotoColorEmoji
        1F004 - 1F1FC  (emojis)

    DejaVuSans (book)
        0000 - FFFD    (lower stuff)


pyftsubset noto/NotoColorEmoji.ttf --unicodes="1F000-1FFFF" --layout-features='*'  --symbol-cmap --legacy-cmap --no-notdef-glyph --no-notdef-outline --output-file=EmojiOnly.ttf
