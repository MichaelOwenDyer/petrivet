#import "base.typ": *

#let theorem(title: none, body) = {
  block(
    width: 100%,
    inset: (left: 0pt, top: 3pt, bottom: 3pt),
    stroke: none,
  )[
    #if title != none [
      #text(weight: "bold")[#title.]
    ]
    #body
    #h(1fr)
    #sym.qed
  ]
}

#let definition(title: none, body) = theorem(title: if title != none { "Definition (" + title + ")" } else { "Definition" }, body)
#let lemma(title: none, body) = theorem(title: if title != none { "Lemma (" + title + ")" } else { "Lemma" }, body)
#let corollary(title: none, body) = theorem(title: if title != none { "Corollary (" + title + ")" } else { "Corollary" }, body)

#let attributed-quote(body, attribution) = {
  pad(left: 1.5em)[
    #block(inset: (left: 1em))[#body]
    #align(right)[(#attribution)]
  ]
}

#let show-listing-styles() = {
  show raw: set text(font: "Courier New", size: 10pt)
  show raw.where(block: true): it => block(
    width: 100%,
    inset: 8pt,
    fill: luma(248),
    radius: 2pt,
    it,
  )
  show raw.line: it => {
    if it.text.starts-with("#") {
      text(fill: tum-elfenbein, it)
    } else {
      it
    }
  }
}
