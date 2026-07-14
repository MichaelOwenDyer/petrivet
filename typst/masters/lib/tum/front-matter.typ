#import "base.typ": *
#import "page.typ": *
#import "headings.typ": *
#import "figures.typ": list-of-figures, list-of-tables

#let titlepages(meta) = {
  titlepage-page(meta)[]
}

#let acknowledgement-page(body) = {
  empty-page[
    #set text(font: body-font, size: body-size, lang: "en")
    #v(6cm)
    #align(center)[#body]
  ]
}

#let abstract-page(body) = {
  pagebreak()
  set page(
    header: context {
      align(right)[#counter(page).display()]
    },
    footer: none,
    numbering: "1",
    number-align: right + top,
  )
  set text(font: body-font, size: body-size, lang: "en")
  set par(
    leading: body-leading,
    spacing: body-par-spacing,
    first-line-indent: 0pt,
    justify: true,
  )
  apply-heading-styles()

  heading(outlined: true, bookmarked: true)[Abstract]
  body
}

#let document-front-matter() = {
  pagebreak()
  outline(
    title: heading(outlined: false, bookmarked: true)[Contents],
    depth: 4,
  )
  list-of-tables()
  list-of-figures()
  pagebreak()
}

// Declaration at the end of the document (matches tex/main.pdf page 10).
#let declaration-page(meta) = {
  pagebreak()
  empty-page[
    #set text(font: body-font, size: body-size, lang: "en")
    #set par(
      leading: body-leading,
      spacing: body-par-spacing,
      first-line-indent: 0pt,
      justify: false,
    )

    #align(center)[
      #text(size: 20pt, weight: "bold")[
        Declaration of Academic Integrity
      ]
    ]
    #v(10mm)

    I hereby declare that the thesis submitted is my own unaided work. All direct or indirect sources used are acknowledged as references.

    I am aware that the thesis in digital form can be examined for the use of unauthorized aid and in order to determine whether the thesis as a whole or parts incorporated in it may be deemed as plagiarism. For the comparison of my work with existing sources I agree that it shall be entered in a database where it shall also remain after examination, to enable comparison with future theses submitted. Further rights of reproduction and usage, however, are not granted here.

    This thesis was not previously presented to another examination board and has not been published.

    #v(30mm)
    #grid(
      columns: (1fr, auto),
      align: (left, right),
      [#meta.hood, #meta.at("submission-date")],
      [#meta.author],
    )
  ]
}

// Backwards-compatible alias.
#let disclaimer = declaration-page
