#import "base.typ": *

#let titlepage-header(meta) = pad(top: 1cm)[
  #set text(font: sans-font, size: 9pt, fill: tum-blau)
  #grid(
    columns: (1fr, auto),
    align: (left + top, right + top),
    box(width: 100%)[
      #meta.chair \
      #meta.department \
      #meta.school \
      #meta.at("university")
    ],
    image(tum-brand-path("Universitaet_Logo_RGB.pdf"), height: tum-logo-height),
  )
]

#let titlepage-foreground = place(
  bottom + right,
  dx: -2 * tum-logo-height,
  dy: -tum-logo-height,
  image(tum-brand-path("TUM_Uhrenturm.png"), width: 11cm),
)

// Single full title page (matches tex/main.pdf — not the duplicate in tum-book.cls).
#let titlepage-page(meta, body) = {
  page(
    paper: "a4",
    margin: tum-margin,
    header: titlepage-header(meta),
    header-ascent: tum-logo-height + 1cm,
    footer: none,
    numbering: none,
    foreground: titlepage-foreground,
  )[
    #set text(font: sans-font, size: body-size, lang: "en")
    #set par(leading: 1em, spacing: 0.3cm, first-line-indent: 0pt)

    #v(0.5cm)
    #text(size: 17pt, weight: "bold")[#meta.title]
    #v(0.3cm)
    #text(size: 14pt)[#meta.at("german-title")]
    #v(0.3cm)
    #text(size: 14pt, weight: "bold", fill: tum-blau)[#meta.author]
    #v(1cm)
    #text(size: 14pt)[#title-description(meta)]
    #v(1cm)
    #text(size: body-size)[#fine-print(meta)]

    #v(1fr)
    #body
  ]
}

#let empty-page(body) = {
  page(
    paper: "a4",
    margin: tum-margin,
    header: none,
    footer: none,
    numbering: none,
  )[#body]
}
