#import "../../lib/tum/lib.typ": float-foot

#figure(
  table(
    columns: (1fr, auto, auto),
    align: (left, center, right),
    table.header([*Value 1*], [*Value 2*], [*Value 3*]),
    [$alpha$], [$beta$], [$gamma$],
    table.hline(),
    [1], [1110.1], [a],
    [2], [10.1], [b],
    [3], [23.113231], [c],
  ),
  caption: [Your first table],
) <tab:table1>

#float-foot[A note describing the table.]

#pagebreak()

#figure(
  image("../../figures/thesis/Universitaet_Flaggen.jpg", width: 70%),
  caption: [My Figure Caption],
) <fig:example>

#float-foot[A note describing the figure.]
