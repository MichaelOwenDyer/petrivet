#import "base.typ": *

#let apply-figure-styles() = {
  show figure.where(kind: table): set figure.caption(position: top)
  show figure.caption: set text(size: 10pt)
  show figure.caption: it => {
    let supplement = it.supplement
    let number = context {
      let kind = it.element.kind
      let ctr = counter(figure.where(kind: kind))
      ctr.display(it.numbering, at: it.element.location())
    }
    block(width: 100%, spacing: 15pt)[
      #align(left)[
        #text(weight: "bold")[#supplement #number]
        #linebreak()
        #text(style: "italic")[#it.body]
      ]
    ]
  }
}

#let float-foot(body) = {
  v(0.5em)
  text(size: 10pt)[#body]
}

#let list-of-tables() = {
  outline(
    title: heading(outlined: false, bookmarked: true)[List of Tables],
    target: figure.where(kind: table),
  )
}

#let list-of-figures() = {
  outline(
    title: heading(outlined: false, bookmarked: true)[List of Figures],
    target: figure.where(kind: image),
  )
}
