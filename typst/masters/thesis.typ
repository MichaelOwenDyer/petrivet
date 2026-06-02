#import "thesis_template.typ": *
#import "common/cover.typ": *
#import "common/titlepage.typ": *
#import "thesis_typ/disclaimer.typ": *
#import "thesis_typ/acknowledgement.typ": *
#import "thesis_typ/abstract_en.typ": *
#import "thesis_typ/abstract_de.typ": *
#import "common/metadata.typ": *


#cover(
  title: titleEnglish,
  degree: degree,
  program: program,
  author: author,
)

#titlepage(
  title: titleEnglish,
  titleGerman: titleGerman,
  degree: degree,
  program: program,
  supervisor: supervisor,
  advisors: advisors,
  author: author,
  startDate: startDate,
  submissionDate: submissionDate
)

#disclaimer(
  title: titleEnglish,
  degree: degree,
  author: author,
  submissionDate: submissionDate
)

#acknowledgement()

#abstract_en()

#abstract_de()

#show: project.with(
  title: titleEnglish,
  titleGerman: titleGerman,
  degree: degree,
  program: program,
  supervisor: supervisor,
  advisors: advisors,
  author: author,
  startDate: startDate,
  submissionDate: submissionDate
)

= Brainstorming
Have to put everything not being generated first, everything that needs to be generated last. 

Orders:
template -> solution -> test -> *ps*
ps -> solution -> test -> *template*
ps -> template -> test -> *solution*
ps -> template -> solution -> *test*

With the code repositories, we have two cases:
- Only parts of the repository being generated (just one file? allow multiple files?)
-- LLM can only modify existing files
- Entire repository being generated
-- LLM can create new files - how to handle this?

= Files we can remove from the code repositories before prompting
- .gitignore
- .gitattributes
- gradlew
- gradlew.bat
- (gradle wrapper directory) gradle/wrapper
- (test) readme.md
-- This is probably unnecessary with fine-tuning as the model should already understand everything in it

= Difficulties with plain prompting
Exercises are just too big; including one example in the prompt already takes upwards of 24k tokens, leaving no room for the actual exercise at hand.
- That means that we are left with explaining the technical details of exercise generation in the prompt, such as:
-- How to mark the test cases on each task in the problem statement
-- How to create UML diagrams in the problem statement
-- How to use the Ares testing framework

= Fine tuning
- Training data should be varied enough to enable the model to generate different kinds of exercises
-- Even mix of English and German exercises
-- Exercises with and without UML diagrams
-- Exercises with different programming languages (Java, Python, OCaml, etc)
-- Lots of variation in the exercise themes
-- Lots of variation in the exercise size and complexity
-- Tests should include behavioral, structural, and unit tests

= Priority
- Get the quality of the problem statements up
- Work on test generation
- Integrate into Artemis
-- Splitting of the generated code into files
-- Chat window on exercise edit page

= Status 23.09
- We decided recently to switch away from multiple different prompts to one single prompt to enable a more gpt-like chat experience in the UI
- The single-prompt approach is showing signs of working well, but the biggest problem is response time
- Guidance is also causing problems, bugs and flaky behavior + cryptic error messages
- One problem: Azure OpenAI occasionally responds with a content-less message, which causes guidance to crash
- Current topic of interest: Best way to get change suggestions from the model
-- Regenerating the entire content is inefficient with respect to time and tokens, especially when the content grows large and there is only a small change
-- We could try to get the model to generate a diff with the suggested changes, however this is not trivial
--> The model tends to generate a lot of garbage when generating diffs, also the diff is not always correct and cannot be automatically applied
--> The model only has a limited amount of reasoning power, and generating a diff requires a lot of reasoning that takes away from the actual content generation (source: https://community.openai.com/t/how-to-generate-automatically-applicable-file-diffs-with-chatgpt/227822/17)
-- Possibility: use the git conflict syntax to indicate changes like in aider: https://github.com/paul-gauthier/aider/blob/cfc7b060df318fccc216c077634b8529094ed02a/coder.py
-- Current approach: Get the model to identify an exact quote from the current content, and then generate a replacement quote -> then we can just do a simple find-and-replace
-- Not yet known how we will handle duplicate matches, maybe we can just take the first one
-- Need a good shortcut for the model to indicate that it wants to replace the entire content, for now we are telling it to respond with "\$" (might want to find a better character, one that is even less likely to appear in any content)
-- Should it be possible for the model to delete or rename files?
-- As of right now, we are showing the entire content of the component to the model in advance, which allows the model to have a lot of context. However we may run into problems with this when the content grows large, since the model will have a lot of potentially irrelevant context to process. Maybe would be better to show the model filepaths only, and have it open them on demand?

= Status 25.09
- We have split up the unified prompt into multiple prompts while keeping the idea that the AI should decide which components to edit
-- This has the benefit of being able to show the instructor a response first and then the changes to the components afterwards when they are ready, instead of everything coming in all at once
-- The plan is now: the model sees the exercise and chat history, and then decides whether it is able to make changes or not. It generates a response for the instructor and if it wants to make changes, it states the specific components and a plan for each one. The components are in order of priority and the changes for each one are fetched in order with the corresponding plan as input.
-- The changes must be fetched linearly in order otherwise they could become inconsistent with one another.
IDEA: Could possibly allow instructor influence on this process -> show in the chat window which components are being generated and which ones are left to go; instructor could cancel/retry individual ones and/or change the order. Instructor could also see and edit the plan for an individual component.

= Status 27.09
- In the process of implementing the server-side code. Xinyao is focusing on the UI and REST API. Coordinating with Timor because he is also doing a lot of work on Iris.
- Have a few questions to bring up with Timor on Friday:

= Status 01.11
- We tried for a long time to inject changes into the code editor directly, but were never successful. Then Patrick told us that we can just set the changes on the server side without actually committing them, and reload the client. This has been much easier
- Biggest challenges right now:
  - AI takes a long time to respond. Is it possible to speed this up with better prompting?
  - Our strategy of having it quote the original text in order to replace it is only seeing about 30-50% accuracy - often it will make a little tiny mistake in the quote and then we can't apply the replacement
  - Tried adding a "band-aid" solution to give it two chances to quote if the first one was not in the text, but this is still not working well. We need to find a better way to get it to quote the original text
  - Thinking of more reliable ways to get the model to specify a location in the text to replace. Maybe first few words and last few words? Fewer words to quote means fewer chances to misquote, but also it might be harder for the model to name two different locations in the text that way

= Status 03.11
- The system is working!
- We've changed strategies for quoting the problem statement to start and end of quote, and this is working better, usually around 60-80% accuracy. Still not perfect though. Biggest things that trip it up are the markdown back-ticks around words which it likes to leave out (\`BubbleSort\`), this still causes quotes to be unreliable. Also, with this new approach, it is noticeably worse at maintaining line breaks and spaces, which means the user has to do a lot of cleanup after it. We're also still deciding whether to use an inclusive or exclusive end quote - currently exclusive, seems to confuse it sometimes.
- For whatever reason, direct quoting seems to work perfectly fine in the code repositories. We are thankful for this, because specifying the start and end of a quote in a code file would be a nightmare (lots of curly braces at the end of code blocks, all looks the same). One thing that it tends to do wrong is add \`\`\`java \`\`\` to the starts and ends of the files, as if the code is in markdown. At least this is easy to remove. We asked it to replace all instances of BubbleSort with QuickSort and it did very well, adding all the necessary classes and modifying existing ones, but there were a handful of BubbleSort instances it forgot about. Nothing that another try couldn't fix. Still need to add the ability to delete a file, that shoulnd't be too hard.

= Status 04.11
- Added the ability to delete and rename files.
- One issue: when switching repositories the currently executing plan stops executing because it is reloaded, and we don't store the execution on the server. It would not be a good idea to make plan execution be server-driven because the user might click away from the page and the server would continue executing the plan anyway. However we still need a way to make it so that the execution state is remembered when switching repositories.
- Another problem: The AI tends to conflate the last few user messages together when generating plans; it will create a plan which contains the user's request as well as the user's previous request. It is valuable to show the AI the conversation history, but we have to discourage this.

Hi Elena, das ist meine Schuld. Ich habe aus Versehen eine alte Gradle-Version in der Aufgabe stehen lassen. Wenn du auf gradle/wrapper/gradle-wrapper.properties gehst, kannst du die Version auf 8.3 ändern. Dann sollte es funktionieren.


= Introduction
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Introduce the topic of your thesis, e.g. with a little historical overview.
]

== Problem
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Describe the problem that you like to address in your thesis to show the importance of your work. Focus on the negative symptoms of the currently available solution.
]

== Motivation
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Motivate scientifically why solving this problem is necessary. What kind of benefits do we have by solving the problem?
]

== Objectives
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Describe the research goals and/or research questions and how you address them by summarizing what you want to achieve in your thesis, e.g. developing a system and then evaluating it.
]

== Outline
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Describe the outline of your thesis
]

= Background
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Describe each proven technology / concept shortly that is important to understand your thesis. Point out why it is interesting for your thesis. Make sure to incorporate references to important literature here.
]

== e.g. User Feedback
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: This section would summarize the concept User Feedback using definitions, historical overviews and pointing out the most important aspects of User Feedback.
]

== e.g. Representational State Transfer
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: This section would summarize the architectural style Representational State Transfer (REST) using definitions, historical overviews and pointing out the most important aspects of the architecture.
]

== e.g. Scrum
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: This section would summarize the agile method Scrum using definitions, historical overviews and pointing out the most important aspects of Scrum.
]

= Related Work
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Describe related work regarding your topic and emphasize your (scientific) contribution in contrast to existing approaches / concepts / workflows. Related work is usually current research by others and you defend yourself against the statement: “Why is your thesis relevant? The problem was al- ready solved by XYZ.” If you have multiple related works, use subsections to separate them.
]

= Requirements Analysis
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: This chapter follows the Requirements Analysis Document Template in bruegge2004object. Important: Make sure that the whole chapter is independent of the chosen technology and development platform. The idea is that you illustrate concepts, taxonomies and relationships of the application domain independent of the solution domain! Cite bruegge2004object several times in this chapter.

]

== Overview
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Provide a short overview about the purpose, scope, objectives and success criteria of the system that you like to develop.
]

== Current System
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: This section is only required if the proposed system (i.e. the system that you develop in the thesis) should replace an existing system.
]

== Proposed System
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: If you leave out the section “Current system”, you can rename this section into “Requirements”.
]

=== Functional Requirements
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: List and describe all functional requirements of your system. Also mention requirements that you were not able to realize. The short title should be in the form “verb objective”

  - FR1 Short Title: Short Description. 
  - FR2 Short Title: Short Description. 
  - FR3 Short Title: Short Description.
]

=== Nonfunctional Requirements
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: List and describe all nonfunctional requirements of your system. Also mention requirements that you were not able to realize. Categorize them using the FURPS+ model described in bruegge2004object without the category functionality that was already covered with the functional requirements.

  - NFR1 Category: Short Description. 
  - NFR2 Category: Short Description. 
  - NFR3 Category: Short Description.

]

== System Models
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: This section includes important system models for the requirements analysis.
]

=== Scenarios
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: If you do not distinguish between visionary and demo scenarios, you can remove the two subsubsections below and list all scenarios here.

  *Visionary Scenarios*

  Note: Describe 1-2 visionary scenario here, i.e. a scenario that would perfectly solve your problem, even if it might not be realizable. Use free text description.

  *Demo Scenarios*

  Note: Describe 1-2 demo scenario here, i.e. a scenario that you can implement and demonstrate until the end of your thesis. Use free text description.
]

=== Use Case Model
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: This subsection should contain a UML Use Case Diagram including roles and their use cases. You can use colors to indicate priorities. Think about splitting the diagram into multiple ones if you have more than 10 use cases. *Important:* Make sure to describe the most important use cases using the use case table template (./tex/use-case-table.tex). Also describe the rationale of the use case model, i.e. why you modeled it like you show it in the diagram.

]

=== Analysis Object Model
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: This subsection should contain a UML Class Diagram showing the most important objects, attributes, methods and relations of your application domain including taxonomies using specification inheritance (see bruegge2004object). Do not insert objects, attributes or methods of the solution domain. *Important:* Make sure to describe the analysis object model thoroughly in the text so that readers are able to understand the diagram. Also write about the rationale how and why you modeled the concepts like this.

]

=== Dynamic Model
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: This subsection should contain dynamic UML diagrams. These can be a UML state diagrams, UML communication diagrams or UML activity diagrams.*Important:* Make sure to describe the diagram and its rationale in the text. *Do not use UML sequence diagrams.*
]

=== User Interface
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Show mockups of the user interface of the software you develop and their connections / transitions. You can also create a storyboard. *Important:* Describe the mockups and their rationale in the text.
]

= System Design
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: This chapter follows the System Design Document Template in bruegge2004object. You describe in this chapter how you map the concepts of the application domain to the solution domain. Some sections are optional, if they do not apply to your problem. Cite bruegge2004object several times in this chapter.
]

== Overview
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Provide a brief overview of the software architecture and references to other chapters (e.g. requirements analysis), references to existing systems, constraints impacting the software architecture..
]

== Design Goals
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Derive design goals from your nonfunctional requirements, prioritize them (as they might conflict with each other) and describe the rationale of your prioritization. Any trade-offs between design goals (e.g., build vs. buy, memory space vs. response time), and the rationale behind the specific solution should be described in this section
]

== Subsytem Decomposition
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Describe the architecture of your system by decomposing it into subsys- tems and the services provided by each subsystem. Use UML class diagrams including packages / components for each subsystem.
]

== Hardware Software Mapping
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: This section describes how the subsystems are mapped onto existing hardware and software components. The description is accompanied by a UML deployment diagram. The existing components are often off-the-shelf components. If the components are distributed on different nodes, the network infrastructure and the protocols are also described.
]

== Persistent Data Management
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Optional section that describes how data is saved over the lifetime of the system and which data. Usually this is either done by saving data in structured files or in databases. If this is applicable for the thesis, describe the approach for persisting data here and show a UML class diagram how the entity objects are mapped to persistent storage. It contains a rationale of the selected storage scheme, file system or database, a description of the selected database and database administration issues.
]

== Access Control
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Optional section describing the access control and security issues based on the nonfunctional requirements in the requirements analysis. It also de- scribes the implementation of the access matrix based on capabilities or access control lists, the selection of authentication mechanisms and the use of en- cryption algorithms.
]

== Global Software Control
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Optional section describing the control flow of the system, in particular, whether a monolithic, event-driven control flow or concurrent processes have been selected, how requests are initiated and specific synchronization issues
]

== Boundry Conditions
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Optional section describing the use cases how to start up the separate components of the system, how to shut them down, and what to do if a component or the system fails.
]

= Case Study / Evaluation
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: If you did an evaluation / case study, describe it here.
]

== Design 
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Describe the design / methodology of the evaluation and why you did it like that. E.g. what kind of evaluation have you done (e.g. questionnaire, personal interviews, simulation, quantitative analysis of metrics, what kind of participants, what kind of questions, what was the procedure?
]

== Objectives
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Derive concrete objectives / hypotheses for this evaluation from the general ones in the introduction.
]

== Results
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Summarize the most interesting results of your evaluation (without interpretation). Additional results can be put into the appendix.
]

== Findings
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Interpret the results and conclude interesting findings
]

== Discussion
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Discuss the findings in more detail and also review possible disadvantages that you found
]

== Limitations
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Describe limitations and threats to validity of your evaluation, e.g. reliability, generalizability, selection bias, researcher bias
]

= Summary
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: This chapter includes the status of your thesis, a conclusion and an outlook about future work.
]

== Status
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Describe honestly the achieved goals (e.g. the well implemented and tested use cases) and the open goals here. if you only have achieved goals, you did something wrong in your analysis.
]

=== Realized Goals
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Summarize the achieved goals by repeating the realized requirements or use cases stating how you realized them.
]

=== Open Goals
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Summarize the open goals by repeating the open requirements or use cases and explaining why you were not able to achieve them. Important: It might be suspicious, if you do not have open goals. This usually indicates that you did not thoroughly analyze your problems.
]

== Conclusion
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Recap shortly which problem you solved in your thesis and discuss your *contributions* here.
]

== Future Work
#rect(
  width: 100%,
  radius: 10%,
  stroke: 0.5pt,
  fill: yellow,
)[
  Note: Tell us the next steps (that you would do if you have more time). Be creative, visionary and open-minded here.
]