#import "base.typ": *
#import "headings.typ": apply-heading-styles, apply-outline-styles
#import "figures.typ": apply-figure-styles

#let tum-thesis = {
  set page(
    paper: "a4",
    margin: tum-margin,
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
  show math.equation: set text(font: body-font, weight: 400)

  show bibliography: set heading(
    outlined: true,
    bookmarked: true,
    numbering: none,
  )

  apply-heading-styles()
  apply-figure-styles()
  apply-outline-styles()
}

#let appendix(body) = {
  pagebreak()
  heading(outlined: true, bookmarked: true)[Appendix]
  body
}
