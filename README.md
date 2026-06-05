# petrivet

`petrivet` is a Rust library for modeling, simulating, and analyzing [Petri nets](https://en.wikipedia.org/wiki/Petri_net).

The library aims to provide an API which is readable and intuitive, even for users who are not familiar with Rust,
backed by an implementation that is highly performant and based on the latest research in the field of Petri nets.

*"Bringing Petri net theory and application together"* is the motto of `petrivet`, and I hope that it can be a useful
tool for both researchers and practitioners working with Petri nets.

`petrivet` is still in early development, and the API is subject to change.
However, I welcome contributions and feedback from the community to help shape the future of the library.

## Features

- **Modeling**: Create and manipulate Petri net models using `NetBuilder`, which provides a fluent API for constructing nets.
- **Loading and saving**: Load and save Petri net models in the [PNML format](https://en.wikipedia.org/wiki/Pnml).
- **Simulation**: Add an initial marking to a `Net` to get a `PetriNet`, which can be simulated by firing transitions and observing the resulting markings.
- **State-space exploration**: Construct either a reachability graph or a coverability graph from a `PetriNet`, and analyze properties such as reachability, coverability, k-boundedness, deadlock-freedom, and liveness.
- **Structural analysis**: System too complex for state-space exploration? `petrivet` also provides structural analysis techniques to prove various properties without having to explore the state space.
- **Structural subclasses**: Check whether a `Net` belongs to a specific structural subclass of Petri nets, such as free-choice nets, marked graphs, or state machines. `petrivet` intelligently chooses the most efficient algorithms available for your net's structure.
- **Performance**: `petrivet` is designed to be fast and efficient. It uses cache-friendly data structures to handle large and complex Petri nets.

## Roadmap

- **Weighted arcs and place capacities**: These two simple extensions to the basic Petri net model are widely used in practice,
and while they are "just" syntactic sugar, they can make a substantial difference in the modeling process.
I plan to add support for these features in the near future.
- **Analysis of unbounded systems**: Currently, `petrivet` returns an inconclusive response when analyzing reachability, liveness, or deadlock-freedom for unbounded systems.
Although these problems are decidable, their worst-case complexity is [Ackermannian](https://en.wikipedia.org/wiki/Ackermann_function) (!), which makes them impractical to solve in general.
However, other model checking tools have implemented various heuristics and optimizations to handle many real-world cases, and I plan to explore similar techniques for `petrivet`.
- **More Petri net extensions**: Dozens of extensions to the basic Petri net model have been proposed in the literature, adding features such as time, probabilities, inhibitor and reset arcs, arbitrary token types ([CPNs](https://en.wikipedia.org/wiki/Coloured_Petri_net)), and much more.
While I don't plan to support all of these extensions, I will consider adding support for the most widely used ones.

## Contributing

Contributions to `petrivet` are very welcome! If you have an idea for a new feature, a bug fix, or an improvement to the documentation, please feel free to open an issue or submit a pull request.

## License

`petrivet` is licensed under the GNU LGPLv3 License. See the [LICENSE](LICENSE) file for more details.
