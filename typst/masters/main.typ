#import "templates/metadata.typ": *
#import "templates/cover.typ": *
#import "templates/titlepage.typ": *
#import "templates/disclaimer.typ": *
#import "templates/acknowledgement.typ": *
#import "templates/abstract_en.typ": *
#import "templates/body.typ": *
#import "content/introduction.typ"

#cover(
  title: title,
  degree: degree,
  program: program,
  author: author,
)

#titlepage(
  title: title,
  degree: degree,
  program: program,
  supervisor: supervisor,
  advisors: advisors,
  author: author,
  startDate: startDate,
  submissionDate: submissionDate
)

#disclaimer(
  title: title,
  degree: degree,
  author: author,
  submissionDate: submissionDate
)

#acknowledgement()

#abstract_en()

#show: body.with(
  title: title,
  degree: degree,
  program: program,
  supervisor: supervisor,
  advisors: advisors,
  author: author,
  startDate: startDate,
  submissionDate: submissionDate
)

#bibliography("content/bibliography.bib")

Introduction
- first research question: what are others doing in this area (model checkers, library)
- second: how should it be implemented? answered by solution design and impl
- third: how can we evaluate it? answered by evaluation

Write Contribution at the end -> link to the Github repo

Methodology: start with design science in information systems (Hevner). How do we write a thesis? Waterfall model, stakeholders, ..
- two subchapters: 1. background: what is design science? What options are there? 2. what is relevant for me? What techniques should I use?

Evaluation: "I am planning a quantitative evaluation of the tool, but I am also planning to do a qualitative evaluation of the tool".
- Describe evaluation parameters and methodology

Structure:
- overview of chapters

Related work:
two subchapters:
1. Foundations: Petri nets, model checking, all kinds of fundamental concepts that are relevant -> Murata paper, other papers
2. Direct: Other implementations to compare with, and highlight what I am doing differently. Structured literature review: how did I find these? What Google scholar keywords did I use to find these? Table search terms -> hits -> selected -> reason for selection -> summary of the paper

Solution Design:
Description of the solution without mentioning any implementation tools (Language etc).
Should have requirements and higher-level design for meeting those requirements.

Implementation:
- Tricks specific to Rust

Evaluation:
lots of graphs

Discussion:
discuss evaluation findings, pick up the research questions again and give short summaries what we found out, link to previous chapters where we answered them.
Strengths and weaknesses of the solution and implementation.

Conclusion:
short summary again
Future work