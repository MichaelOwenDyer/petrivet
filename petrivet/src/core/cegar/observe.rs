use crate::core::cegar::lemma::IdxLemma;
use crate::core::mapping::DenseMapping;
use crate::core::marking::IdxMarking;
use crate::core::parikh_vector::IdxParikhVector;
use crate::system::observe::CegarEvent;
use std::sync::{Arc, mpsc};

/// An `IdxCegarEvent` is emitted by the CEGAR-based analysis whenever a spurious candidate
/// is found and a lemma is derived to eliminate it.
#[derive(Debug, Clone)]
pub struct IdxCegarEvent {
    /// The marking which the SMT solver thought was a possible solution,
    /// but was actually spurious.
    pub spurious_marking: IdxMarking<u32>,
    /// The Parikh vector which the SMT solver thought was a possible solution,
    /// but was actually spurious. This is only present if transition variables
    /// have been added to the SMT problem.
    pub spurious_parikh_vector: Option<IdxParikhVector<u32>>,
    /// The lemma which was generated to eliminate this spurious solution.
    pub lemma: IdxLemma,
}

/// A type-erased sink for [`IdxCegarEvent`]s.
pub type CegarCallbackFn = Box<dyn Fn(IdxCegarEvent) + Send>;

/// A trait for converting a public-facing [`mpsc::Sender<CegarEvent>`] into a private-facing
/// [`CegarCallbackFn`] which handles the translation from index-based to public-facing types
/// via [`DenseMapping`].
pub trait ToCegarCallbackFn {
    fn to_cegar_callback_fn(self, mapping: Arc<DenseMapping>) -> CegarCallbackFn;
}

impl ToCegarCallbackFn for mpsc::Sender<CegarEvent> {
    fn to_cegar_callback_fn(self, mapping: Arc<DenseMapping>) -> CegarCallbackFn {
        Box::new(move |event: IdxCegarEvent| {
            let _ = self.send(mapping.cegar_event(event));
        }) as CegarCallbackFn
    }
}

/// A wrapper around an optional [`CegarCallbackFn`], adding the (marking, Parikh vector) context
/// for the current step, so refinement rules can invoke it once per [`IdxLemma`] they assert.
pub struct CegarObserver {
    pub callback: Option<CegarCallbackFn>,
}

impl CegarObserver {
    /// Bind (marking, Parikh vector) context for the current step, returning a callback that
    /// refinement rules can invoke once per [`IdxLemma`] they assert, or `None` if no
    /// observer is registered, so callers can skip building lemmas they'd otherwise discard.
    ///
    /// Returns a boxed trait object rather than `impl Fn(IdxLemma)` for the same reason
    /// [`CegarCallbackFn`] itself is boxed: refinement rules' `encode_into` methods are already
    /// generic over the `SmtSolver` backend, and accepting this by a second generic parameter
    /// would multiply that per distinct call site instead of erasing it once, here, where it's
    /// cheap (one allocation per CEGAR step, not per lemma).
    pub fn with_context(
        &self,
        marking: IdxMarking<u32>,
        parikh_vector: Option<IdxParikhVector<u32>>,
    ) -> Option<Box<dyn Fn(IdxLemma) + '_>> {
        self.callback.as_deref().map(|sink| {
            Box::new(move |lemma| {
                sink(IdxCegarEvent {
                    spurious_marking: marking.clone(),
                    spurious_parikh_vector: parikh_vector.clone(),
                    lemma,
                });
            }) as Box<dyn Fn(IdxLemma) + '_>
        })
    }
}
