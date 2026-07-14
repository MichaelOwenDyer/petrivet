#import "base.typ": *

#let apply-heading-styles() = {
  set heading(numbering: none, bookmarked: true, outlined: true)
  show heading.where(level: 1): set text(
    font: body-font,
    size: 17pt,
    weight: "bold",
  )
  show heading.where(level: 2): set text(
    font: body-font,
    size: body-size,
    weight: "bold",
  )
  show heading.where(level: 3): set text(
    font: body-font,
    size: body-size,
    weight: "bold",
    style: "italic",
  )
  show heading.where(level: 1): set block(above: 12pt, below: 0pt)
  show heading.where(level: 2): set block(above: 12pt, below: 0pt)
  show heading.where(level: 3): set block(above: 12pt, below: 0pt)
  show heading.where(level: 4): it => {
    box(width: 100%, inset: (left: 1.27cm))[
      #text(weight: "bold")[#it.body.]
      #v(-1.2em)
    ]
  }
}

#let apply-outline-styles() = {
  show outline.entry.where(level: 1): set text(weight: "bold")
  show outline.entry.where(level: 1): set block(above: 2em, below: 0pt)
  show outline.entry.where(level: 2): set block(above: 0.5em, below: 0pt)
}
