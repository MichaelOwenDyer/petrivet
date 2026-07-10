*"Making Petri net theory applied"*

# petrivet

`petrivet` is a Rust library for modeling, simulating, and analyzing [Petri nets](https://en.wikipedia.org/wiki/Petri_net).

The library aims to provide a readable and intuitive API backed by an implementation that strives to be as performant and correct as possible.

I am writing about `petrivet` in my ongoing Master's thesis at the Technical University of Munich.
I hope that it can be a useful tool for researchers and engineers alike.

`petrivet` is being actively developed. The API is subject to change at any time, but is gradually converging to a stable state.

## Features

- **Modeling**: Create and manipulate Petri net models using `NetBuilder`, which provides a fluent API for constructing nets.

- **Simulation**: Add an initial marking to a `Net` to get a `PetriNet`, which can be simulated by firing transitions and observing the resulting markings.

- **State-space exploration**: Explore a Petri net's reachability graph or Karp-Miller coverability graph. Choose between breadth-first and depth-first search.

- **Structural analysis**: Check various structural properties of a net to derive important insights into its behavior.

- **Structural subclasses**: Various insights from the structure of a net alone (without any initial marking) allow us to derive more efficient procedures than in the general case.
`petrivet` strives to achieve complexity parity with the theoretical lower bound of all algorithms it implements.

- **Performance**: `petrivet` strives to be as performant and memory-efficient as possible, but is still far behind state-of-the-art model checkers like [ITS-Tools](https://github.com/lip6/ITSTools/tree/master), [tedd](https://projects.laas.fr/tina/index.php), [SMTP](https://github.com/nicolasamat/SMPT), and [tapaal](https://www.tapaal.net/). I submitted a very immature version of `petrivet` in 05.2026 to the 2026 Model Checking Contest, and came in last place as expected (see the [results](https://mcc.lip6.fr/2026/results.php)). But some significant progress is being made on this front.

- **PNML Import / Export**: Load and save Petri net models in the [PNML format](https://en.wikipedia.org/wiki/Pnml). Disclaimer: this feature is largely AI-written and deserves some more love, but I've had other priorities so far.

## Roadmap

- *More sophisticated procedures* for reachability and coverability using constraint solvers ([SMT](https://en.wikipedia.org/wiki/Satisfiability_modulo_theories), [Integer Programming](https://en.wikipedia.org/wiki/Integer_programming)). These technologies are used heavily in state-of-the-art model checkers. They allow us to construct a very simple approximation of a Petri net as a system of equations, and gradually add more detail until, hopefully, the solver can gather enough information from the simplified model to answer the question about the Petri net at hand without further expensive analysis. But even the most state-of-the-art procedures can do nothing about the astronomical theoretical complexity in the worst case.
- **Weighted arcs and place capacities**: These two simple extensions to the basic Petri net model are widely used in practice and can prove extremely useful syntactic sugar for certain modeling tasks. The feature is currently blocked on some important design questions for the core data layout and how to ensure correctness of structural analysis.
- **Analysis of unbounded systems**: Petri nets which can accumulate an unbounded number of tokens pose a much greater analysis challenge than bounded Petri nets. `petrivet` is lacking some functionality in this area: it lacks a proper decision procedure for reachability in unbounded systems, which also blocks deadlock-freedom and liveness. Unbounded reachability has one of the highest known complexities of any decidable problem, in the worst-case [Ackermannian](https://en.wikipedia.org/wiki/Ackermann_function), but it is still decidable. Despite that, petrivet can currently only prove reachability, not disprove it - the previously mentioned sophisticated SMT solver procedures may alleviate this in the near future.
- **API Extensibility**: Dozens if not hundreds of extensions to the basic Petri net model have been proposed over the years, adding features such as time, probabilities, inhibitor and reset arcs, arbitrary token types ([CPNs](https://en.wikipedia.org/wiki/Coloured_Petri_net)), and much more. While petrivet still only implements ordinary Petri nets, a long-term goal of the library is an extensibility concept in the API and the first-party implementation of various popular extensions.
- For more TODOs, see the [GitHub issues page](https://github.com/MichaelOwenDyer/petrivet/issues).

## Contributing

Contributions to `petrivet` are very welcome! If you have an idea for a new feature, a bug fix, or an improvement to the documentation, please feel free to open an issue or submit a pull request.

## License

`petrivet` is licensed under the GNU LGPLv3 License. See the [LICENSE](LICENSE) file for more details.
