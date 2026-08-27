*"Making Petri net theory applied"*

# petrivet

`petrivet` is a Rust library for modeling, simulating, and analyzing [Petri nets](https://en.wikipedia.org/wiki/Petri_net).

`petrivet` aims to provide an ergonomic and intuitive API backed by an implementation that is as performant and correct as possible.
Its documentation is also intended to be a valuable educational resource for Petri net theory, with many examples and explanations of the underlying concepts.
I hope that it can be a useful tool for researchers and engineers alike.

`petrivet` is being actively developed. The API is subject to change at any time, but is gradually converging to a stable state.

## Features

- **Modeling**: Create and manipulate Petri net models using `NetBuilder`, which provides a fluent API for constructing nets.

- **Simulation**: Add an initial marking to a `Net` to get a `PetriNet`, which can be simulated by firing transitions and observing the resulting markings.

- **State-space exploration**: Explore a Petri net's reachability graph or Karp-Miller coverability graph. Choose between breadth-first and depth-first search.

- **Behavioral properties**: Check (k-)boundedness, deadlock-freedom, liveness, coverability, and reachability.

- **Structural properties**: Check place and transition invariants, siphons and traps, the Commoner-Hack Criterion, and more.

- **PNML Import / Export**: Load and save Petri net models in the [PNML format](https://en.wikipedia.org/wiki/Pnml). Disclaimer: this feature is largely AI-written and deserves some more love, but I've had other priorities so far.

Currently, only ordinary Petri nets are supported, without any extensions such as arc weights, place capacities, et cetera.

## Roadmap

- **Weighted arcs and place capacities**: These two simple extensions to the basic Petri net model are widely used in
practice and can prove extremely useful syntactic sugar for certain modeling tasks. The feature is currently blocked on
some important design questions for the core data layout and how to ensure correctness of structural analysis.
- **API Extensibility**: Dozens of extensions to the basic Petri net model have been proposed over the years, 
adding features such as time, probabilities, inhibitor and reset arcs, arbitrary token types ([CPNs](https://en.wikipedia.org/wiki/Coloured_Petri_net)),
and much more. While petrivet still only implements ordinary Petri nets, a long-term goal of the library is
an extensibility concept in the API and the first-party implementation of various popular extensions.
- For more TODOs, see the [GitHub issues page](https://github.com/MichaelOwenDyer/petrivet/issues).

## Contributing

Contributions to `petrivet` are very welcome! If you have an idea for a new feature, a bug fix, or an improvement to the documentation, please feel free to open an issue or submit a pull request.

## License

`petrivet` is licensed under the GNU LGPLv3 License. See the [LICENSE](LICENSE) file for more details.
