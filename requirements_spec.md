# Petrivet: Software Requirements Specification

## 1. Introduction

### 1.1. Purpose
This document outlines the software requirements for **petrivet**, a comprehensive Rust library for the modeling, simulation, and analysis of Petri nets.

### 1.2. Vision
The vision for petrivet is to become the de facto standard, high-performance, and ergonomic library for working with Petri nets in the Rust ecosystem.
It will provide a robust framework for academic research, industrial applications, and educational purposes by offering a strongly-typed, modular, and extensible API and support for various Petri net classes and analysis techniques.

## 2. Core Design Principles

The library's API will be heavily reliant on Rust's trait system and type-state programming paradigm to enforce correctness at compile time.
Different types of Petri nets and their associated properties will be represented by distinct types or type combinations, with analysis methods implemented as trait methods available only to valid net constructions.

[//]: # (- **Structural Properties**: A tuple of sets `&#40;S, T, F&#41;` &#40;Places, Transitions, Flow Relation&#41; will grant access to traits for fundamental structural analysis.)

[//]: # (- **Capacity and Weight**: A tuple `&#40;S, T, F, K, W&#41;` representing a net with place capacities and arc weights will expose different trait implementations for analysis that considers these factors.)

[//]: # (- **Behavioral Properties**: A tuple `&#40;N, M0&#41;` representing a net N with an initial marking M0 will provide access to traits for behavioral analysis &#40;e.g., reachability&#41;.)

- **Equivalence via Newtype Pattern**: The library will use the newtype pattern to wrap complex nets (e.g., those with capacities/weights) to behave as an equivalent ordinary net where applicable. This will enable the use of theorems and algorithms defined only for ordinary Petri nets on a wider class of nets.

- **Generic Token Counting**: The library will support generic unsigned integer types for token counting, allowing users to choose the most memory-efficient representation for their specific use case (u8 for small nets, u128 for nets requiring very large token counts).

- **Performance-First Identifiers**: Internal implementations will avoid string-based identifiers in favor of type-safe numeric identifiers to maximize performance and prevent runtime errors.

- **Zero-Cost Abstractions for Metadata**: The library will provide optional metadata capabilities that incur absolutely no performance or memory cost when unused. Users who need only algorithmic functionality can use the core types, while users who need names, descriptions, or other metadata can opt into separate metadata-aware types.

## 3. Core Features and Functional Requirements
This section outlines the specific functional capabilities the library will provide.
All Petri net classes will, at the very minimum, be "simulatable" in the sense that the user can execute firing sequences and observe the resulting markings.
Beyond that, the library will provide a rich set of analysis tools for both structural and behavioral properties of Petri nets, in particular reachability, coverability, liveness, boundedness, and deadlock-freedom.
Different net classes will have different complexity characteristics for these properties, and the library will make a best effort to optimize analysis by leveraging structural properties and known theorems where applicable.
Where appropriate, the library will provide traits for the various properties that can be analyzed, with implementations that leverage the specific structural features of the net class to optimize analysis.

### 3.1. Structural Analysis
The library shall provide tools for analyzing the static structure of a Petri net.

#### 3.1.1. Incidence Matrix
The library must be able to compute and provide access to the incidence matrix of a given P/T net.

#### 3.1.2. S-Invariants
Implementation of algorithms to compute the S-invariants (place invariants) of a net.

#### 3.1.3. T-Invariants
Implementation of algorithms to compute the T-invariants (transition invariants) of a net.

#### 3.1.4. Structural Subclass Identification
The library will provide traits and methods to determine if a net's structure conforms to well-known subclasses, including:
- State Machines
- Marked Graphs  
- Free Choice Nets

#### 3.1.5. Siphon and Trap Analysis
The library shall include algorithms for computing minimal siphons and traps. This is a prerequisite for advanced liveness analysis, such as applying Commoner's Theorem for Free Choice nets.

#### 3.1.6. Advanced Structural Properties
The library will compute additional structural properties that provide insights into net behavior.

- **Synchronic Distance**: Measure the maximum token imbalance between places
- **Structural Conflicts**: Identify places with multiple output transitions (choice points)
- **Structural Concurrency**: Identify transitions that can fire concurrently
- **Net Decomposition**: Decompose nets into strongly connected components or modules
- **Reduction Rules**: Apply structural reduction techniques (fusion, elimination, etc.)

#### 3.1.7. Matrix-Based Analysis
Provide comprehensive matrix-based structural analysis tools.

- **Pre/Post Incidence Matrices**: Compute separate pre and post incidence matrices
- **Flow Matrix**: Compute the combined flow matrix (post - pre)
- **Rank Analysis**: Determine the rank of incidence matrices for invariant analysis
- **Kernel Computation**: Compute null spaces for invariant generation
- **Matrix Decomposition**: Perform LU, QR, or SVD decomposition for analysis

### 3.2. Behavioral and Reachability Analysis
The library shall provide tools for analyzing the dynamic behavior of a marked Petri net.

#### 3.2.1. Marking Equation
The library will provide an interface to represent and solve the state (marking) equation of a net. This may involve integrating with external SMT/linear algebra solvers (e.g., z3-rs).

#### 3.2.2. State Space Exploration

The library shall provide rich graph structures for state space exploration, not just simple iterators. These structures will support both manual iteration and high-level query methods.

##### Coverability Graph
The `CoverabilityGraph` struct represents the finite coverability tree/graph with ω-markings:

- **Graph Construction**: Lazy construction that builds the graph on-demand as nodes are explored
- **Iterator Implementation**: Implements standard Rust iterator traits for manual traversal
- **Query Methods**: High-level methods that internally progress the iterator as needed:
  - `is_coverable(marking)`: Determines if a marking is coverable
  - `find_covering_marking(marking)`: Finds a marking in the graph that covers the target
  - `get_witness_path(marking)`: Returns a firing sequence that reaches a covering marking
  - `is_bounded()`: Determines if the net is bounded (no ω symbols in final graph)
- **Termination Guarantee**: Construction always terminates due to ω-acceleration
- **Graph Access**: Provide access to the underlying graph structure for advanced analysis

##### Reachability Graph
The `ReachabilityGraph` struct represents the potentially infinite reachability graph:

- **Bounded Exploration**: Configurable bounds to ensure termination:
  - Maximum number of states to explore
  - Maximum search depth from initial marking
  - Timeout-based termination
  - Memory usage limits
- **Iterator Implementation**: Implements iterator traits with termination safeguards
- **Query Methods with Termination**: All query methods include termination mechanisms:
  - `is_reachable_bounded(marking, max_states)`: Reachability with state limit
  - `is_reachable_within_depth(marking, max_depth)`: Reachability within depth limit
  - `is_reachable_timeout(marking, timeout)`: Reachability with time limit
  - `find_shortest_path(marking, max_depth)`: Shortest path with depth bound
  - `explore_up_to(limit)`: Explore graph up to specified limit
- **Partial Results**: Methods return rich result types indicating:
  - Definitive answer (reachable/unreachable)
  - Inconclusive due to termination bounds
  - Statistics about exploration (states visited, depth reached)
- **Resumable Exploration**: Ability to resume exploration from previous stopping point

##### Advanced State Space Features

- **Rackoff's Algorithm**: An implementation of Rackoff's algorithm (or a similarly bounded algorithm) shall be provided to determine if a specific marking is coverable and to provide a bound on the size of reachable markings.

- **Backwards Reachability Analysis**: The library will support backwards reachability analysis to find a set of initial markings that can reach a given target marking (or a marking that covers it).

- **Incremental Construction**: Both graph types support incremental construction, allowing efficient repeated queries without rebuilding from scratch.

- **Graph Serialization**: Ability to serialize/deserialize partially or fully constructed graphs for persistence and analysis.

##### Termination and Resource Management

- **Configurable Limits**: All exploration methods accept limit parameters:
  ```rust
  pub struct ExplorationLimits {
      max_states: Option<usize>,
      max_depth: Option<usize>,
      max_memory: Option<usize>,
      timeout: Option<Duration>,
  }
  ```

- **Progress Callbacks**: Support for progress callbacks to monitor exploration:
  ```rust
  pub trait ExplorationCallback {
      fn on_state_discovered(&mut self, state: &Marking, depth: usize);
      fn should_continue(&self) -> bool;
  }
  ```

- **Resource Monitoring**: Built-in monitoring of memory usage and computation time with automatic termination when limits are exceeded.

#### 3.2.3. Cumulative Liveness and Boundedness Analysis
The library shall provide a mechanism to cumulatively analyze and report on the liveness and boundedness properties of a net. This system will maintain the 'best known' state for each property (e.g., unbounded / _k_-bounded; L0 / L1 / L2 / L3 / L4 liveness) and refine its conclusions as more information is gathered from various analysis techniques, including:

- Structural properties (e.g., a net being structurally bounded)
- T-Invariant analysis
- Siphon/Trap properties (e.g., presence of a marked trap)
- Partial results from the reachability/coverability iterators

#### 3.2.4. Advanced Reachability Queries
The library shall support sophisticated queries about the reachability space beyond basic marking reachability.

- **Path Queries**: Find execution sequences that lead from one marking to another
- **Witness Generation**: Provide concrete firing sequences that demonstrate reachability
- **Unreachability Proofs**: Generate certificates proving that certain markings are unreachable  
- **Conditional Reachability**: Determine reachability under specific constraints or assumptions
- **Minimal Path Finding**: Find shortest firing sequences between markings

#### 3.2.5. Temporal and Modal Logic (Reach Goal)
Support for temporal logic queries over Petri net execution traces.

- **Linear Temporal Logic (LTL)**: Evaluate LTL formulas over firing sequences
- **Computation Tree Logic (CTL)**: Support CTL queries over the reachability graph
- **Fairness Constraints**: Incorporate fairness assumptions in temporal reasoning
- **Property Templates**: Provide common temporal property patterns (safety, liveness, etc.)

### 3.3. Support for Specialty and High-Level Nets
The library will extend beyond simple P/T nets to support more expressive models.

#### 3.3.1. Common Net Classes
The library will provide convenient constructors or types for common subclasses of nets, such as E/C nets (Event-Condition nets with fixed arc weights and place capacities of 1).

#### 3.3.2. Extended P/T Nets
Support for modeling and analyzing nets with extensions like:

- **Inhibitor Arcs**: Arcs that prevent a transition from firing if the input place is marked.
- **Reset Arcs**: Arcs that empty a place of all its tokens when a transition fires.

#### 3.3.3. Colored Petri Nets (CPNs)
The library will provide a framework for creating and analyzing Colored Petri Nets.

- **Generic Token Types**: Token data will be representable by generic types (`<T>`).
- **Transition Guards and Functions**: Transitions will be associated with functions (closures) that operate on token data to determine if the transition is enabled and to compute the output markings.

#### 3.3.4. Algorithmic Petri Nets (Reach Goal)

The library will provide foundational support for Algorithmic Petri Nets (APNs), a high-level Petri net formalism where transitions are associated with algorithms or procedures, enabling the modeling of complex, data-driven, or computational behaviors within the net.

- **Transition Algorithms**: Each transition in an APN can be associated with a user-defined function or algorithm, which may inspect and manipulate tokens, perform computations, or interact with external systems.
- **Token Data**: Tokens may carry structured or typed data, and transitions may read, modify, or produce tokens based on algorithmic logic.
- **Guarded Execution**: Transitions may include guards—predicates over token data or net state—that determine their enablement.
- **Side Effects**: The APN framework will allow controlled side effects (e.g., I/O, state mutation) within transition algorithms, with clear boundaries to ensure analysis remains tractable where possible.
- **Extensibility**: The API will be designed to allow users to define custom transition behaviors using closures or trait objects, leveraging Rust's type system for safety and expressiveness.

**Analysis Support**: While full formal analysis of APNs may be undecidable in general, the library will provide best-effort simulation, step-by-step execution, and limited static analysis (e.g., detection of pure/impure transitions, or static guards).



### 3.4. Simulation and Execution
The library shall provide comprehensive simulation capabilities for executing Petri nets step-by-step or continuously.

#### 3.4.1. Step-by-Step Execution
- **Manual Firing**: Allow users to manually fire specific enabled transitions
- **Transition Selection**: Provide methods to query which transitions are currently enabled
- **State Inspection**: Allow inspection of current marking at any point during execution
- **Execution History**: Optionally maintain a history of fired transitions and intermediate markings

#### 3.4.2. Automatic Simulation
- **Random Firing**: Implement random transition selection from enabled transitions
- **Priority-Based Firing**: Support transition priorities for deterministic simulation
- **Timed Simulation**: Support for timed Petri nets with transition delays
- **Termination Conditions**: Configurable stopping criteria (step count, time limit, deadlock detection)

#### 3.4.3. Simulation Statistics
- **Throughput Analysis**: Track transition firing frequencies and rates
- **Place Occupancy**: Monitor token distribution and place utilization over time
- **Bottleneck Detection**: Identify transitions or places that limit system performance
- **Cycle Detection**: Detect and analyze cyclic behavior in the net execution

### 3.5. Import and Export Capabilities
The library shall support standard formats for interoperability with other Petri net tools.

#### 3.5.1. Standard Format Support
- **PNML (Petri Net Markup Language)**: Full import/export support for the ISO/IEC 15909 standard
- **DOT Format**: Export to Graphviz DOT format for visualization
- **JSON/YAML**: Lightweight serialization formats for web applications
- **Custom Binary Format**: High-performance serialization for large nets

#### 3.5.2. Validation and Compatibility
- **Format Validation**: Ensure imported nets conform to expected schemas
- **Version Compatibility**: Handle different versions of standard formats
- **Lossy Conversion Warnings**: Alert users when format conversions lose information
- **Metadata Preservation**: Maintain layout, colors, and annotations where possible

### 3.6. Visualization and Debugging Support
The library shall provide utilities to support visualization and debugging workflows.

#### 3.6.1. Graph Layout Algorithms
- **Force-Directed Layout**: Automatic positioning using spring-force algorithms
- **Hierarchical Layout**: Layered layouts for workflow-like nets
- **Circular Layout**: Arrange nodes in circles for cyclic structures
- **Manual Layout**: Support for user-specified node positions

#### 3.6.2. Debugging Utilities
- **Deadlock Analysis**: Detect and explain deadlock conditions
- **Unreachable Transition Detection**: Identify transitions that can never fire
- **Marking Validation**: Verify that markings are valid for the net structure
- **Invariant Checking**: Validate that net execution preserves known invariants

### 3.7. Performance Analysis and Optimization
The library shall provide tools for analyzing and optimizing net performance.

#### 3.7.1. Complexity Analysis
- **State Space Size Estimation**: Provide bounds on reachability graph size
- **Algorithmic Complexity Reporting**: Report time/space complexity of analysis operations
- **Scalability Metrics**: Measure how analysis performance scales with net size

#### 3.7.2. Optimization Suggestions
- **Structural Simplification**: Suggest equivalent but simpler net structures
- **Redundancy Detection**: Identify and suggest removal of redundant places or transitions
- **Parallelization Opportunities**: Identify independent subnetworks for parallel analysis

### 3.8. Subclass-Specific Implementations
Leveraging the structural identification in 3.1.4, the library will provide specialized and more efficient algorithm implementations for certain net subclasses. For example, properties like liveness and boundedness can be decided more efficiently for Free Choice nets (using Commoner's Theorem) or structurally bounded nets, avoiding full state space exploration where possible.

#### 3.8.1. Workflow Nets
- **Soundness Checking**: Verify workflow net soundness properties
- **Completion Analysis**: Ensure all cases can reach proper completion
- **Resource Analysis**: Analyze resource requirements and bottlenecks

## 4. Implementation Priorities and Advanced Features

### 4.0. Development Priority Classification
The library features are classified into priority levels to guide development:

#### 4.0.1. HIGH PRIORITY - Core Performance Architecture
These features are essential for the library's performance goals and must be implemented early:

1. **Const Generic Nets (Section 4.3)**: Stack-allocated markings and compile-time optimizations
2. **Declarative Macro (Section 5.5.2)**: Essential for const generic net construction  
3. **Spatial Data Structures**: KD-Tree and related optimizations for coverability analysis
4. **Dual Net Architecture**: Supporting both dynamic and const generic net types

**Rationale**: These features are interdependent and provide the foundation for the library's performance advantages. The macro is required to make const generics practical, and spatial data structures leverage the const generic architecture for maximum benefit.

#### 4.0.2. MEDIUM PRIORITY - Core Functionality
Standard Petri net analysis features:

1. **Basic Structural Analysis** (Section 3.1): Incidence matrices, invariants, subclass identification
2. **Reachability/Coverability Analysis** (Section 3.2): State space exploration with termination safeguards
3. **Builder API** (Section 5.5.1): Runtime net construction
4. **Import/Export** (Section 3.5): PNML and other standard formats

#### 4.0.3. LOWER PRIORITY - Advanced Features

### 4.1. Parallel Exploration
As a reach goal, the library could offer a parallel implementation for exploring the coverability/reachability tree. This would involve distributing the exploration of different branches across multiple threads using libraries like Rayon. Such a feature could significantly speed up analysis for certain classes of nets and could be used to power real-time applications, such as a 3D visualization of the tree's growth.

### 4.2. Support for Generalized Petri Nets
The library could be extended to support Generalized Petri Nets with integer weights and markings (G-Markings), allowing for negative token counts. This would require implementing specialized analysis concepts, such as i-coverability and i-r-coverability, to reason about the state space of such nets.

### 4.3. Compile-Time Known Nets (const Generics) - HIGH PRIORITY
As a high-priority performance optimization, the library will provide specialized net types where the structure (number of places, transitions, and arcs) is known entirely at compile time. This enables significant performance improvements through:

#### 4.3.1. Stack-Allocated Data Structures
- **Stack-Allocated Markings**: Use `[T; N_PLACES]` instead of heap-allocated `Vec<T>`, providing substantial speedups for simulation of small-to-medium sized nets
- **Cache Locality**: Improved memory access patterns due to contiguous stack allocation
- **Zero Allocation Overhead**: Eliminate heap allocation costs during marking operations
- **Compile-Time Bounds Checking**: Prevent runtime bounds checking overhead

#### 4.3.2. Specialized Algorithm Implementations
- **Optimized Firing Rules**: Unrolled loops and compile-time optimizations for transition enabling checks
- **SIMD Opportunities**: Fixed-size arrays enable better auto-vectorization by the compiler
- **Inlined Operations**: Small, fixed-size operations can be aggressively inlined

#### 4.3.3. Const Generic Net Type
```rust
/// A Petri net with compile-time known structure for maximum performance
pub struct ConstOrdinaryNet<const N_PLACES: usize, const N_TRANSITIONS: usize, T: Unsigned = u32> {
    // Fixed-size array-based storage optimized for the specific dimensions
    inputs: [[PlaceId; MAX_INPUTS]; N_TRANSITIONS],  // or similar optimized representation
    place_to_outputs: [[TransitionId; MAX_OUTPUTS]; N_PLACES],
    // Metadata about actual sizes within the fixed arrays
}

/// Stack-allocated marking for const generic nets
pub type ConstMarking<T, const N_PLACES: usize> = [T; N_PLACES];
```

#### 4.3.4. Spatial Data Structure Integration
For const generic nets, the library will leverage spatial data structures to optimize coverability graph construction:

- **KD-Tree Integration**: Use KD-Trees or similar spatial structures to optimize marking coverage queries, potentially reducing complexity from O(N²) to O(log N) for the "have we seen a covering marking?" check
- **Dimension-Aware Optimization**: Choose optimal spatial data structures based on the number of places (N_PLACES)
- **Covering Relation Queries**: Efficient spatial queries to find all discovered markings M' where M' ≤ M for a given marking M
- **Alternative Structures**: For high-dimensional nets, consider R-Trees, LSH, or other structures that handle the curse of dimensionality better than KD-Trees

### 4.4. Arbitrary Node Data Attachment (Reach Goal)
As an advanced feature, the library should support attaching arbitrary user data to places and transitions without impacting the performance of core Petri net algorithms. This data should be:

- **Transparently Separate**: User data storage should be completely separate from the hot-path data structures used during simulation and analysis
- **Type-Safe**: Users should be able to attach strongly-typed data of their choice
- **Optional**: The core library should function efficiently even when no user data is attached
- **Accessible**: Provide ergonomic APIs to access and modify attached data using the same `PlaceId` and `TransitionId` handles

This feature would enable use cases such as:
- Attaching visualization coordinates to nodes
- Storing semantic information for domain-specific modeling
- Associating debugging or profiling metadata
- Linking to external systems or databases

**Implementation Strategy**: Use a separate hash map or similar structure that maps from node IDs to user data, ensuring that the core algorithms never need to traverse or consider this additional data.

### 4.5. WebAssembly (WASM) Support (Reach Goal)
As a reach goal, the library should compile to WebAssembly to enable Petri net analysis and simulation directly in web browsers. This would enable:

- **Interactive Web Applications**: Browser-based Petri net editors and simulators
- **Educational Tools**: Web-based learning platforms for Petri net theory
- **Distributed Computing**: Client-side analysis to reduce server load
- **Cross-Platform Deployment**: Single codebase running on desktop and web

The library should be designed with WASM compatibility in mind, avoiding features that don't compile to WASM and ensuring that performance characteristics remain acceptable in the browser environment.

### 4.6. Testing and Quality Assurance
The library shall maintain exceptional quality through comprehensive testing and continuous performance monitoring.

#### 4.6.1. Unit Test Coverage
The library must achieve and maintain high unit test coverage across all components:

- **Coverage Target**: Minimum 95% line coverage, 90% branch coverage
- **Algorithm Testing**: Every analysis algorithm must have comprehensive test cases covering:
  - Correct results on known examples from literature
  - Edge cases (empty nets, single-node nets, disconnected components)
  - Boundary conditions (maximum token counts, large net sizes)
  - Error conditions and invalid inputs
- **Property-Based Testing**: Use property-based testing frameworks (e.g., `proptest`) to validate invariants:
  - Structural properties remain consistent after net modifications
  - Analysis results are deterministic for the same inputs
  - Serialization/deserialization round-trips preserve net structure
- **Cross-Platform Testing**: Ensure all tests pass on major platforms (Linux, macOS, Windows) and architectures (x86_64, ARM64)

#### 4.6.2. Integration and System Testing
Beyond unit tests, the library requires comprehensive integration testing:

- **End-to-End Workflows**: Test complete analysis pipelines from net construction to result interpretation
- **Format Compatibility**: Validate import/export with real-world Petri net files from other tools
- **Memory Safety**: Use tools like `miri` and `valgrind` to detect memory safety issues
- **Fuzz Testing**: Employ fuzzing to discover edge cases in parsing and analysis algorithms
- **WASM Testing**: Dedicated test suite for WebAssembly builds to ensure browser compatibility

#### 4.6.3. Performance Benchmarking
Rigorous performance testing shall prevent regressions and guide optimizations:

- **Benchmark Suite**: Comprehensive benchmarks covering:
  - Net construction (builder API, parsing from formats)
  - Core algorithms (reachability analysis, invariant computation, structural analysis)
  - Simulation performance (transition firing rates, large state spaces)
  - Memory usage patterns (allocation patterns, peak memory consumption)
  - Scalability (performance vs. net size, token counts, analysis depth)

- **Regression Detection**: Automated performance regression detection:
  - Baseline performance metrics stored in version control
  - CI/CD integration to detect performance regressions before merging
  - Configurable thresholds for acceptable performance changes
  - Historical performance tracking and visualization

- **Real-World Benchmarks**: Performance testing on realistic Petri nets:
  - Academic benchmark suites (Model Checking Contest nets)
  - Industrial workflow models
  - Protocol specifications and communication models
  - Large-scale nets (1000+ places/transitions)

#### 4.6.4. Continuous Integration and Quality Gates
Automated quality assurance processes:

- **CI Pipeline Requirements**:
  - All tests must pass on multiple Rust versions (MSRV + stable + nightly)
  - Performance benchmarks must not regress beyond acceptable thresholds
  - Code coverage reports generated and tracked over time
  - Documentation builds successfully and examples compile
  - WASM builds compile and pass browser-specific tests

- **Quality Gates**:
  - No pull request may be merged with failing tests
  - Performance regressions require explicit justification and approval
  - New features must include corresponding tests and documentation
  - Breaking API changes require version bump and migration guide

#### 4.6.5. Testing Infrastructure
Specialized testing infrastructure to support comprehensive validation:

- **Test Data Management**: Curated collection of test nets including:
  - Canonical examples from Petri net literature
  - Pathological cases designed to stress algorithms
  - Randomly generated nets with known properties
  - Real-world models from various application domains

- **Performance Test Environment**: Dedicated, consistent hardware for benchmarking:
  - Isolated environment to minimize performance variance
  - Multiple hardware configurations (different CPU/memory combinations)
  - Automated result collection and historical comparison

- **Correctness Oracles**: Reference implementations and known results:
  - Cross-validation against established Petri net tools
  - Hand-verified results for critical test cases
  - Theoretical bounds and expected complexity characteristics

### 4.7. Documentation and Examples
Comprehensive documentation ensures the library is accessible to both novice and expert users.

#### 4.7.1. API Documentation
- **Complete Coverage**: Every public function, struct, and trait must have comprehensive rustdoc documentation
- **Mathematical Foundations**: Include mathematical definitions and references to relevant literature
- **Complexity Information**: Document time and space complexity for all algorithms
- **Usage Examples**: Provide code examples for every major API component
- **Cross-References**: Link related concepts and alternative approaches

#### 4.7.2. Educational Materials
- **Tutorial Series**: Progressive tutorials from basic concepts to advanced analysis
- **Worked Examples**: Detailed walkthroughs of real-world Petri net analysis problems
- **Algorithm Explanations**: Intuitive explanations of complex algorithms with visualizations
- **Best Practices Guide**: Recommendations for efficient and correct usage patterns

#### 4.7.3. Example Collection
- **Comprehensive Examples**: Curated collection of example programs demonstrating:
  - Basic net construction and analysis
  - Advanced analysis techniques
  - Integration with visualization tools
  - Performance optimization strategies
  - Domain-specific applications (workflows, protocols, etc.)
- **Executable Documentation**: All examples must compile and run as part of CI
- **Interactive Examples**: Web-based examples using WASM builds for browser execution

## 5. Core Data Structures and API Design (Ordinary Nets)
This section specifies the primary data structures for representing an ordinary Petri net `(S, T, F)`. The design prioritizes performance and memory efficiency for simulation and analysis algorithms.

### 5.0. Token Type Flexibility
The library shall provide generic support for different unsigned integer types to represent token counts. This allows users to optimize memory usage and performance based on their specific requirements:

- **Small nets** (≤255 tokens per place): `u8` for minimal memory footprint
- **Medium nets** (≤65,535 tokens per place): `u16` for balanced performance
- **Standard nets** (≤4.3 billion tokens per place): `u32` as the default choice
- **Large-scale nets**: `u64` or `u128` for extreme cases

All token-counting types must implement the `Unsigned` trait from the `num-traits` crate to ensure consistent arithmetic operations. The library will provide type aliases for common configurations to improve ergonomics.

### 5.1. Core Types and Identifiers
To ensure type safety and prevent logic errors, strong typing will be used for identifiers via the newtype pattern.

```rust
// Represents a unique identifier for a Place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct PlaceId(usize);

// Represents a unique identifier for a Transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct TransitionId(usize);

/// Represents an arc in the Petri net.
/// Provides ergonomic construction from tuples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arc {
    PlaceToTransition(PlaceId, TransitionId),
    TransitionToPlace(TransitionId, PlaceId),
}

impl From<(PlaceId, TransitionId)> for Arc {
    fn from((place, transition): (PlaceId, TransitionId)) -> Self {
        Arc::PlaceToTransition(place, transition)
    }
}

impl From<(TransitionId, PlaceId)> for Arc {
    fn from((transition, place): (TransitionId, PlaceId)) -> Self {
        Arc::TransitionToPlace(transition, place)
    }
}
```

### 5.2. Net Structure Representation
The static structure of an ordinary Petri net will be represented by the `OrdinaryNet` struct. This struct uses a compact, cache-friendly adjacency list model optimized for forward simulation (determining enabled transitions).

```rust
/// Represents the structure of an Ordinary Petri Net.
/// Arc weights are implicitly 1.
/// This core structure contains only the essential algorithmic data.
pub struct OrdinaryNet {
    /// Number of places in the net (for bounds checking and iteration)
    num_places: usize,
    
    /// Number of transitions in the net (for bounds checking and iteration)
    num_transitions: usize,

    /// Maps `TransitionId` to its input `PlaceId`s (the pre-set).
    /// `inputs[t.0]` gives the Vec of places in the pre-set of transition `t`.
    /// This is the critical hot-path structure for firing rules.
    inputs: Vec<Vec<PlaceId>>,

    /// Maps `PlaceId` to its output `TransitionId`s.
    /// Used to efficiently find which transitions are affected by a token
    /// change in a given place.
    /// `outputs[p.0]` gives the Vec of transitions that have `p` as an input.
    place_to_outputs: Vec<Vec<TransitionId>>,
}
```

**Design Rationale:**

- **`Vec<Vec<PlaceId>>` for inputs**: This is the most critical structure for the firing rule. It allows direct, O(1) lookup of a transition's pre-set.

- **`place_to_outputs`**: While not strictly necessary for firing, this "reverse" mapping is crucial for efficiently determining which transitions to re-check for enabledness after a place's token count changes. Without it, we would have to check every single transition in the net on every firing.

- **Numeric IDs over String IDs**: All internal algorithms operate exclusively on `PlaceId` and `TransitionId` numeric handles. String names are relegated to optional metadata that exists solely for user convenience. This design choice provides several benefits:
  - **Performance**: Numeric comparisons and hash operations are faster than string operations
  - **Memory Efficiency**: Numeric IDs are smaller and more cache-friendly
  - **Type Safety**: Compile-time prevention of mixing place and transition identifiers
  - **Determinism**: Numeric IDs provide consistent ordering and behavior

- **Complete Separation**: Algorithmic data structures are completely independent of user-facing features, ensuring that performance-critical operations never need to consider non-essential data.

### 5.3. Marking Representation
A marking is a state of the net. For performance, it will be a simple vector of token counts, directly indexable by `PlaceId`. The token count type is generic to allow users to choose the most appropriate unsigned integer type for their use case.

```rust
/// Represents a marking (state) of an Ordinary Petri Net.
/// The vector's length is equal to the number of places in the net.
/// T must implement the Unsigned trait from num-traits for token counting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marking<T: Unsigned>(Vec<T>);
```

### 5.4. Marked Net Representation
A net combined with its initial state is the entry point for all behavioral analysis. The marking type is generic to match the token counting type chosen by the user.

```rust
/// An ordinary net with an initial marking.
/// T must implement the Unsigned trait from num-traits for token counting.
pub struct MarkedOrdinaryNet<T: Unsigned = u32> {
    net: OrdinaryNet,
    initial_marking: Marking<T>,
}
```

### 5.5. Net Construction
Constructing a net's graph structure manually is verbose and error-prone. The library will provide two primary methods for net creation: a flexible Builder API for runtime construction and a declarative macro for compile-time construction.

#### 5.5.1. Type-Safe Builder API
The primary interface for constructing nets at runtime will be the `PetriNetBuilder`. This API is designed to be type-safe by returning opaque `PlaceId` and `TransitionId` handles, which are then used for subsequent operations. This prevents errors such as referencing non-existent nodes. The API is kept minimal and focused purely on structural construction.

```rust
pub struct PetriNetBuilder {
    // Internal state for the builder.
    // ... details omitted from spec ...
}

impl PetriNetBuilder {
    /// Creates a new, empty builder.
    pub fn new() -> Self { /* ... */ }

    /// Adds a new place to the net, returning a safe handle to it.
    pub fn add_place(&mut self) -> PlaceId { /* ... */ }

    /// Adds a new transition, returning a safe handle.
    pub fn add_transition(&mut self) -> TransitionId { /* ... */ }

    /// Adds N places to the net, returning an array of safe handles.
    /// The number of places is determined by the const generic parameter N.
    /// Example: let [p1, p2, p3] = builder.add_places::<3>();
    pub fn add_places<const N: usize>(&mut self) -> [PlaceId; N] { /* ... */ }

    /// Adds N transitions to the net, returning an array of safe handles.
    /// The number of transitions is determined by the const generic parameter N.
    /// Example: let [t1, t2] = builder.add_transitions::<2>();
    pub fn add_transitions<const N: usize>(&mut self) -> [TransitionId; N] { /* ... */ }

    /// Creates an arc in the net.
    /// Accepts any type that implements Into<Arc>, enabling ergonomic syntax:
    /// - add_arc((place_id, transition_id)) for place-to-transition arcs
    /// - add_arc((transition_id, place_id)) for transition-to-place arcs
    pub fn add_arc<A: Into<Arc>>(&mut self, arc: A) { /* ... */ }

    /// Consumes the builder to produce a validated OrdinaryNet.
    /// This method will perform final validation and optimize the data structures
    /// for analysis and simulation.
    pub fn build(self) -> Result<OrdinaryNet, BuildError> { /* ... */ }
}
```

**Example Usage:**
```rust
// Basic usage
let mut builder = PetriNetBuilder::new();
let p1 = builder.add_place();
let p2 = builder.add_place();
let t1 = builder.add_transition();

// Ergonomic arc creation using tuples
builder.add_arc((p1, t1));  // Place-to-transition arc
builder.add_arc((t1, p2));  // Transition-to-place arc

let net = builder.build().unwrap();

// Batch creation example
let mut builder = PetriNetBuilder::new();
let [input, buffer, output] = builder.add_places::<3>();
let [produce, consume] = builder.add_transitions::<2>();

// Chain of arcs
builder.add_arc((input, produce));
builder.add_arc((produce, buffer));
builder.add_arc((buffer, consume));
builder.add_arc((consume, output));

let net = builder.build().unwrap();

// Complex pipeline example
let mut builder = PetriNetBuilder::new();
let [p1, p2, p3, p4, p5] = builder.add_places::<5>();
let [t1, t2, t3, t4] = builder.add_transitions::<4>();

// Build a linear pipeline
for (place, transition) in [(p1, t1), (p2, t2), (p3, t3), (p4, t4)] {
    builder.add_arc((place, transition));
}
for (transition, place) in [(t1, p2), (t2, p3), (t3, p4), (t4, p5)] {
    builder.add_arc((transition, place));
}

let net = builder.build().unwrap();
```

**Note on Metadata**: The builder API focuses purely on structural construction for maximum simplicity and performance. Metadata (names, descriptions, user data) can be attached separately using dedicated metadata management APIs or external mapping structures, ensuring that users who don't need metadata face no API complexity or performance overhead.

### 5.7. State Space Graph Structures
The library provides rich graph structures for state space exploration that combine iterator functionality with high-level query methods.

#### 5.7.1. Coverability Graph Structure
```rust
/// Represents the coverability graph with ω-markings for boundedness analysis.
/// Guarantees termination through ω-acceleration.
pub struct CoverabilityGraph<T: Unsigned = u32> {
    // Internal graph representation
    // ... implementation details omitted ...
}

/// A marking that may contain ω (omega) symbols representing unboundedness
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OmegaPlaceMarking<T: Unsigned> {
    Finite(T),
    Omega,
}

pub type OmegaMarking<T> = Vec<OmegaPlaceMarking<T>>;

impl<T: Unsigned> CoverabilityGraph<T> {
    /// Create a new coverability graph for the given net and initial marking
    pub fn new(net: &OrdinaryNet, initial: &Marking<T>) -> Self { /* ... */ }
    
    /// Check if a specific marking is coverable
    pub fn is_coverable(&mut self, target: &Marking<T>) -> bool { /* ... */ }
    
    /// Find a coverability marking that covers the target (if any)
    pub fn find_covering_marking(&mut self, target: &Marking<T>) -> Option<OmegaMarking<T>> { /* ... */ }
    
    /// Get a witness firing sequence that reaches a covering marking
    pub fn get_witness_path(&mut self, target: &Marking<T>) -> Option<Vec<TransitionId>> { /* ... */ }
    
    /// Determine if the net is bounded (no ω symbols in final graph)
    pub fn is_bounded(&mut self) -> bool { /* ... */ }
    
    /// Get all coverability markings discovered so far
    pub fn discovered_markings(&self) -> &[OmegaMarking<T>] { /* ... */ }
    
    /// Force complete construction of the coverability graph
    pub fn complete_construction(&mut self) { /* ... */ }
}

/// Iterator over already discovered markings (non-mutating)
pub struct DiscoveredMarkingsIter<'a, T: Unsigned> {
    // Internal iterator state - may use advanced data structures
    // ... implementation details omitted ...
}

/// Lazy exploration iterator that mutably borrows the graph
pub struct ExplorationIter<'a, T: Unsigned> {
    graph: &'a mut CoverabilityGraph<T>,
    strategy: ExplorationStrategy,
    // Internal state for lazy exploration
    // ... implementation details omitted ...
}

#[derive(Debug, Clone, Copy)]
pub enum ExplorationStrategy {
    DepthFirst,
    BreadthFirst,
}

impl<T: Unsigned> CoverabilityGraph<T> {
    /// Get an iterator over already discovered markings (does not explore new states)
    pub fn discovered_markings_iter(&self) -> DiscoveredMarkingsIter<T> { /* ... */ }
    
    /// Get a lazy exploration iterator starting from the initial marking
    pub fn explore_from_initial(&mut self, strategy: ExplorationStrategy) -> ExplorationIter<T> { /* ... */ }
    
    /// Get a lazy exploration iterator starting from the current exploration frontier
    pub fn explore_from_current(&mut self, strategy: ExplorationStrategy) -> ExplorationIter<T> { /* ... */ }
    
    /// Reset exploration state to initial marking (for restarting exploration)
    pub fn reset_exploration(&mut self) { /* ... */ }
}
```

#### 5.7.2. Reachability Graph Structure
```rust
/// Represents the potentially infinite reachability graph with termination safeguards.
pub struct ReachabilityGraph<T: Unsigned = u32> {
    // Internal graph representation with exploration state
    // ... implementation details omitted ...
}

/// Result type for reachability queries that may be inconclusive
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReachabilityResult<T> {
    /// Definitively reachable with witness path
    Reachable(Vec<TransitionId>),
    /// Definitively unreachable (for bounded exploration)
    Unreachable,
    /// Inconclusive due to termination limits
    Inconclusive {
        states_explored: usize,
        max_depth_reached: usize,
        reason: TerminationReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminationReason {
    StateLimit,
    DepthLimit,
    TimeLimit,
    MemoryLimit,
    UserCallback,
}

/// Configuration for exploration limits
#[derive(Debug, Clone)]
pub struct ExplorationLimits {
    pub max_states: Option<usize>,
    pub max_depth: Option<usize>,
    pub max_memory_mb: Option<usize>,
    pub timeout: Option<std::time::Duration>,
}

impl<T: Unsigned> ReachabilityGraph<T> {
    /// Create a new reachability graph for the given net and initial marking
    pub fn new(net: &OrdinaryNet, initial: &Marking<T>) -> Self { /* ... */ }
    
    /// Check reachability with state count limit
    pub fn is_reachable_bounded(&mut self, target: &Marking<T>, max_states: usize) -> ReachabilityResult<T> { /* ... */ }
    
    /// Check reachability within depth limit
    pub fn is_reachable_within_depth(&mut self, target: &Marking<T>, max_depth: usize) -> ReachabilityResult<T> { /* ... */ }
    
    /// Check reachability with timeout
    pub fn is_reachable_timeout(&mut self, target: &Marking<T>, timeout: std::time::Duration) -> ReachabilityResult<T> { /* ... */ }
    
    /// Check reachability with comprehensive limits
    pub fn is_reachable_limited(&mut self, target: &Marking<T>, limits: &ExplorationLimits) -> ReachabilityResult<T> { /* ... */ }
    
    /// Find shortest path with depth bound
    pub fn find_shortest_path(&mut self, target: &Marking<T>, max_depth: usize) -> ReachabilityResult<T> { /* ... */ }
    
    /// Explore graph up to specified limit, returning exploration statistics
    pub fn explore_up_to(&mut self, limits: &ExplorationLimits) -> ExplorationStatistics { /* ... */ }
    
    /// Resume exploration from previous stopping point
    pub fn resume_exploration(&mut self, limits: &ExplorationLimits) -> ExplorationStatistics { /* ... */ }
    
    /// Get all markings discovered so far
    pub fn discovered_markings(&self) -> &[Marking<T>] { /* ... */ }
    
    /// Check if exploration has been exhausted (for bounded nets)
    pub fn is_complete(&self) -> bool { /* ... */ }
}

/// Iterator over already discovered markings in reachability graph (non-mutating)
pub struct ReachabilityDiscoveredIter<'a, T: Unsigned> {
    // Internal iterator state - may use advanced data structures
    // ... implementation details omitted ...
}

/// Lazy exploration iterator for reachability graph with termination safeguards
pub struct ReachabilityExplorationIter<'a, T: Unsigned> {
    graph: &'a mut ReachabilityGraph<T>,
    strategy: ExplorationStrategy,
    limits: ExplorationLimits,
    // Internal state for bounded lazy exploration
    // ... implementation details omitted ...
}

impl<T: Unsigned> ReachabilityGraph<T> {
    /// Get an iterator over already discovered markings (does not explore new states)
    pub fn discovered_markings_iter(&self) -> ReachabilityDiscoveredIter<T> { /* ... */ }
    
    /// Get a bounded lazy exploration iterator starting from the initial marking
    pub fn explore_from_initial_bounded(
        &mut self, 
        strategy: ExplorationStrategy, 
        limits: ExplorationLimits
    ) -> ReachabilityExplorationIter<T> { /* ... */ }
    
    /// Get a bounded lazy exploration iterator starting from the current exploration frontier
    pub fn explore_from_current_bounded(
        &mut self, 
        strategy: ExplorationStrategy, 
        limits: ExplorationLimits
    ) -> ReachabilityExplorationIter<T> { /* ... */ }
    
    /// Reset exploration state to initial marking (for restarting exploration)
    pub fn reset_exploration(&mut self) { /* ... */ }
    
    /// Get the current exploration frontier (markings at the boundary of explored space)
    pub fn exploration_frontier(&self) -> Vec<&Marking<T>> { /* ... */ }
}

/// Statistics about graph exploration
#[derive(Debug, Clone)]
pub struct ExplorationStatistics {
    pub states_discovered: usize,
    pub max_depth_reached: usize,
    pub exploration_time: std::time::Duration,
    pub memory_used_mb: usize,
    pub termination_reason: Option<TerminationReason>,
}
```

#### 5.7.3. Progress Monitoring and Callbacks
```rust
/// Trait for monitoring exploration progress
pub trait ExplorationCallback<T: Unsigned> {
    /// Called when a new state is discovered
    fn on_state_discovered(&mut self, state: &Marking<T>, depth: usize, total_discovered: usize);
    
    /// Called periodically to check if exploration should continue
    fn should_continue(&self) -> bool;
    
    /// Called when exploration terminates
    fn on_termination(&mut self, reason: TerminationReason, stats: &ExplorationStatistics);
}

impl<T: Unsigned> ReachabilityGraph<T> {
    /// Explore with callback for progress monitoring
    pub fn explore_with_callback<C: ExplorationCallback<T>>(
        &mut self, 
        limits: &ExplorationLimits,
        callback: &mut C
    ) -> ExplorationStatistics { /* ... */ }
}

// Iterator trait implementations for all iterator types

impl<'a, T: Unsigned> Iterator for DiscoveredMarkingsIter<'a, T> {
    type Item = &'a OmegaMarking<T>;
    
    fn next(&mut self) -> Option<Self::Item> { /* ... */ }
    
    // Provide size_hint if the internal structure supports it
    fn size_hint(&self) -> (usize, Option<usize>) { /* ... */ }
}

impl<'a, T: Unsigned> Iterator for ExplorationIter<'a, T> {
    type Item = OmegaMarking<T>;
    
    fn next(&mut self) -> Option<Self::Item> { 
        // Lazily explore new states according to the strategy
        // Terminates when coverability graph is complete
        /* ... */ 
    }
}

impl<'a, T: Unsigned> Iterator for ReachabilityDiscoveredIter<'a, T> {
    type Item = &'a Marking<T>;
    
    fn next(&mut self) -> Option<Self::Item> { /* ... */ }
    
    fn size_hint(&self) -> (usize, Option<usize>) { /* ... */ }
}

impl<'a, T: Unsigned> Iterator for ReachabilityExplorationIter<'a, T> {
    type Item = Result<Marking<T>, TerminationReason>;
    
    fn next(&mut self) -> Option<Self::Item> { 
        // Lazily explore new states with termination safeguards
        // Returns Ok(marking) for new states, Err(reason) when limits are hit
        /* ... */ 
    }
}

// Additional utility methods for the iterator types

impl<'a, T: Unsigned> ExplorationIter<'a, T> {
    /// Get statistics about the current exploration
    pub fn exploration_stats(&self) -> ExplorationStatistics { /* ... */ }
    
    /// Check if the coverability graph construction is complete
    pub fn is_complete(&self) -> bool { /* ... */ }
}

impl<'a, T: Unsigned> ReachabilityExplorationIter<'a, T> {
    /// Get statistics about the current exploration
    pub fn exploration_stats(&self) -> ExplorationStatistics { /* ... */ }
    
    /// Check if exploration limits have been reached
    pub fn limits_reached(&self) -> Option<TerminationReason> { /* ... */ }
    
    /// Update exploration limits (e.g., extend timeout)
    pub fn update_limits(&mut self, new_limits: ExplorationLimits) { /* ... */ }
}

##### Iterator Usage Patterns

The different iterator types support various exploration patterns:

```rust
// Example usage patterns for CoverabilityGraph
let mut cov_graph = CoverabilityGraph::new(&net, &initial_marking);

// Pattern 1: Iterate over already discovered markings (non-mutating)
for marking in cov_graph.discovered_markings_iter() {
    println!("Already discovered: {:?}", marking);
}

// Pattern 2: Lazy exploration from initial marking
for marking in cov_graph.explore_from_initial(ExplorationStrategy::BreadthFirst) {
    println!("Newly discovered: {:?}", marking);
    // Can break early if desired condition is met
    if some_condition(&marking) {
        break;
    }
}

// Pattern 3: Continue exploration from current frontier
for marking in cov_graph.explore_from_current(ExplorationStrategy::DepthFirst) {
    println!("Continuing exploration: {:?}", marking);
}

// Example usage patterns for ReachabilityGraph  
let mut reach_graph = ReachabilityGraph::new(&net, &initial_marking);
let limits = ExplorationLimits {
    max_states: Some(1000),
    max_depth: Some(50),
    max_memory_mb: Some(100),
    timeout: Some(Duration::from_secs(10)),
};

// Pattern 1: Iterate over already discovered markings
for marking in reach_graph.discovered_markings_iter() {
    println!("Already discovered: {:?}", marking);
}

// Pattern 2: Bounded lazy exploration with error handling
for result in reach_graph.explore_from_initial_bounded(ExplorationStrategy::BreadthFirst, limits.clone()) {
    match result {
        Ok(marking) => {
            println!("Newly discovered: {:?}", marking);
        }
        Err(reason) => {
            println!("Exploration terminated: {:?}", reason);
            break;
        }
    }
}

// Pattern 3: Exploration with statistics monitoring
let mut explorer = reach_graph.explore_from_current_bounded(ExplorationStrategy::DepthFirst, limits);
while let Some(result) = explorer.next() {
    match result {
        Ok(marking) => println!("Found: {:?}", marking),
        Err(reason) => {
            let stats = explorer.exploration_stats();
            println!("Stopped due to {:?}, explored {} states", reason, stats.states_discovered);
            break;
        }
    }
}
```
```

#### 5.5.2. Declarative Macro - HIGH PRIORITY
To support const generic nets and provide a highly ergonomic way to embed fixed Petri nets directly in source code, a procedural macro `petrinet!` is essential. This DSL enables compile-time net construction required for const generic optimization.

##### 5.5.2.1. Const Generic Integration
The macro must generate const generic net types with compile-time known dimensions:

```rust
// --- Enhanced DSL Usage for Const Generics ---
let net: ConstOrdinaryNet<3, 2> = petrinet! {
    places: { p1, p2, p3 }        // 3 places -> N_PLACES = 3
    transitions: { t1, t2 }       // 2 transitions -> N_TRANSITIONS = 2
    arcs: {
        p1 -> t1,
        t1 -> p2,
        p2 -> t2,
        t2 -> p3
    }
};

// The macro automatically infers the const generic parameters
// and generates optimized data structures
```

##### 5.5.2.2. Macro Requirements
- **Compile-Time Analysis**: The macro must analyze the net structure during compilation to determine const generic parameters
- **Type Inference**: Automatically infer `N_PLACES` and `N_TRANSITIONS` from the macro input
- **Validation**: Perform structural validation at compile time (e.g., ensure all referenced places/transitions exist)
- **Optimization Hints**: Generate code optimized for the specific net dimensions
- **Error Messages**: Provide clear compile-time error messages for malformed nets

##### 5.5.2.3. Dual Construction Paths
The library will support two distinct construction approaches:

```rust
// Path 1: Runtime construction with builder (dynamic sizing)
let mut builder = PetriNetBuilder::new();
let p1 = builder.add_place();
// ... runtime construction
let dynamic_net: OrdinaryNet = builder.build()?;

// Path 2: Compile-time construction with macro (const generics)
let const_net: ConstOrdinaryNet<5, 3> = petrinet! {
    // ... compile-time construction
};
```

These serve different use cases:
- **Builder**: For nets whose structure is determined at runtime (user input, file parsing, algorithmic generation)
- **Macro**: For nets whose structure is known at compile time (embedded models, benchmarks, examples)

### 5.6. Architectural Design Decisions for Performance Optimization

#### 5.6.1. Dual Net Type Architecture
The library's architecture will be built around two complementary net representations:

1. **Dynamic Nets (`OrdinaryNet`)**: 
   - Vec-based storage for runtime flexibility
   - Builder-based construction
   - Suitable for nets of unknown size or structure
   - Standard heap allocation patterns

2. **Const Generic Nets (`ConstOrdinaryNet<N_PLACES, N_TRANSITIONS>`)**: 
   - Array-based storage for compile-time optimization
   - Macro-based construction
   - Suitable for nets with known structure
   - Stack allocation and compile-time optimizations

#### 5.6.2. Algorithm Specialization Strategy
Many algorithms will have specialized implementations for const generic nets:

```rust
// Example: Transition enabling check
impl<T: Unsigned> OrdinaryNet {
    fn is_enabled(&self, transition: TransitionId, marking: &Marking<T>) -> bool {
        // Dynamic implementation with bounds checking and heap access
        self.inputs[transition.0].iter()
            .all(|&place_id| marking.0[place_id.0] > T::zero())
    }
}

impl<const N_PLACES: usize, const N_TRANSITIONS: usize, T: Unsigned> 
    ConstOrdinaryNet<N_PLACES, N_TRANSITIONS, T> {
    fn is_enabled(&self, transition: TransitionId, marking: &ConstMarking<T, N_PLACES>) -> bool {
        // Optimized implementation with compile-time bounds checking
        // Potential for loop unrolling and SIMD optimization
        self.inputs[transition.0].iter()
            .all(|&place_id| marking[place_id.0] > T::zero())
    }
}
```

#### 5.6.3. Spatial Data Structure Integration
The const generic architecture enables sophisticated spatial optimizations:

- **Dimension-Aware Selection**: Choose optimal spatial data structures based on `N_PLACES`
- **Stack-Allocated Queries**: Spatial queries can use stack-allocated coordinate arrays
- **Compile-Time Optimization**: Spatial operations can be optimized for specific dimensionality

```rust
impl<const N_PLACES: usize, const N_TRANSITIONS: usize, T: Unsigned> 
    CoverabilityGraph<ConstOrdinaryNet<N_PLACES, N_TRANSITIONS, T>> {
    
    fn find_covering_markings(&self, target: &ConstMarking<T, N_PLACES>) -> Vec<OmegaMarking<T>> {
        match N_PLACES {
            1..=8 => self.kd_tree_query(target),      // KD-Tree for low dimensions
            9..=20 => self.rtree_query(target),       // R-Tree for medium dimensions  
            _ => self.lsh_query(target),              // LSH for high dimensions
        }
    }
}
```

#### 5.6.4. Performance Trade-offs and Usage Guidelines

##### 5.6.4.1. When to Use Const Generic Nets
**Optimal Use Cases:**
- Small to medium-sized nets (≤ 100 places/transitions) where structure is known at compile time
- High-frequency simulation or analysis where performance is critical
- Embedded or real-time applications requiring predictable performance
- Benchmark suites and academic examples with fixed structure
- Inner loops of algorithms that create/analyze many markings

**Performance Benefits:**
- 2-10x faster marking operations due to stack allocation
- Better cache locality and reduced memory fragmentation  
- Elimination of bounds checking overhead
- Compiler optimizations (loop unrolling, SIMD, inlining)
- Predictable memory usage patterns

##### 5.6.4.2. When to Use Dynamic Nets
**Optimal Use Cases:**
- Large nets (> 100 places/transitions) where const generics become unwieldy
- Nets loaded from external files or user input
- Algorithmic net generation where structure is computed at runtime
- Interactive applications where net structure changes frequently
- Prototype development and exploration

**Trade-offs:**
- Heap allocation overhead for markings and data structures
- Runtime bounds checking
- Less aggressive compiler optimizations
- More flexible but potentially slower

##### 5.6.4.3. Hybrid Usage Patterns
The library will support conversion between net types where appropriate:

```rust
// Convert dynamic net to const generic for performance-critical analysis
let dynamic_net: OrdinaryNet = load_from_file("model.pnml")?;
if dynamic_net.num_places() <= 50 && dynamic_net.num_transitions() <= 30 {
    let const_net: ConstOrdinaryNet<50, 30> = dynamic_net.try_into_const()?;
    // Use const_net for high-performance analysis
}

// Use const generic nets for inner loop computations
for marking in reachability_analysis(&const_net) {
    // Fast stack-allocated marking operations
    let next_markings = simulate_step(&const_net, &marking);
}
```