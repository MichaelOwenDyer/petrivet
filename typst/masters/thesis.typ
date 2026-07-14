#import "content/meta.typ": meta
#import "lib/tum/lib.typ": *
#import "content/front/abstract.typ": abstract-body
#import "content/front/acknowledgement.typ": acknowledgement-body

#let thesis() = {
  titlepages(meta)
  acknowledgement-page(acknowledgement-body)
  abstract-page(abstract-body)

  show: tum-thesis
  document-front-matter()

  include "content/chapters/01-introduction.typ"
  include "content/chapters/02-related-work.typ"
  include "content/chapters/03-solution-design.typ"
  include "content/chapters/04-implementation.typ"
  include "content/chapters/05-evaluation.typ"
  include "content/chapters/06-discussion.typ"
  include "content/chapters/07-conclusion.typ"

  pagebreak()
  bibliography("content/back/bibliography.bib", style: "ieee")

  appendix[
    #include "content/back/appendix.typ"
  ]

  declaration-page(meta)
}
