// TUM Corporate Design constants (from tum-base.sty / tum-base-text.sty).
// Title page and declaration text follow tex/main.pdf (chair build), not the
// raw tum-book.cls AtBeginDocument blocks.

#let tum-logo-height = 14mm
#let tum-margin = 2.54 * tum-logo-height

#let tum-blau = rgb("#0065bd")
#let tum-blau-dunkel = rgb("#005293")
#let tum-blau-hell = rgb("#98c6ea")
#let tum-orange = rgb("#e37222")
#let tum-gruen = rgb("#a2ad00")
#let tum-elfenbein = rgb("#dad7cb")

#let body-font = ("Times New Roman", "TeX Gyre Termes", "Libertinus Serif")
#let sans-font = ("Helvetica", "Arial")
#let body-size = 12pt

// LaTeX setspace doublespacing + parskip 15pt at 12pt body.
#let body-leading = 12pt
#let body-par-spacing = 15pt

#let tum-figure-path(path) = "../../figures/thesis/" + path
#let tum-brand-path(path) = "../../figures/tum/" + path

#let title-description(meta) = [
  Thesis \
  for the Attainment of the Degree \
  *#meta.degree* \
  #v(body-par-spacing)

  at the #meta.school, \
  #meta.department, \
  #meta.chair
]

#let fine-print(meta) = {
  let supervisor-2 = meta.at("supervisor-2", default: none)
  let matriculation = meta.at("matriculation-number", default: none)

  [
    *Examiner* \
    #meta.examiner \
    #v(12pt)

    *Supervised by* \
    #meta.supervisor \
    #v(12pt)

    #if supervisor-2 != none and supervisor-2 != "" [
      *Supervisor 2* \
      #supervisor-2 \
      #v(12pt)
    ]

    *Submitted by* \
    #meta.author \
    #if matriculation != none and matriculation != "" [
      \
      *Matriculation Number:* #matriculation
    ]
    #v(12pt)

    *Submitted on* \
    #meta.at("submission-date")
  ]
}
