# Aug 14 2025
# Journal Entry

Trying to figure out how best to separate net structure and behavior in the codebase.
It doesn't make a lot of sense to have a net with weights but firing rules that do not consider the weights.
This makes me think that the net behavior should be derived from the structure.

15.08
I am realizing that the structure and behavior of a PT net can not be completely decoupled, since the types of the annotations
(capacities, weights, resets, inhibitors) are part of the structure, yet must be compatible with the content of the marking in use.
For example, there would be no good definition for a net with a boolean capacities and integer markings.