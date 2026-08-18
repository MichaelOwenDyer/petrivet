use crate::core::state_space::TokenOps;
use crate::net::Transition;

#[derive(Debug, Clone, Default, Eq, PartialEq, Hash)]
pub struct ParikhVector<T> {
    /// The support of the marking.
    ///
    /// Only places with non-zero token counts are stored here,
    /// and a token count of zero is implicitly assigned to all other places.
    pub(crate) support: Vec<(Transition, T)>,
}

impl<T> ParikhVector<T> {
    pub fn support(&self) -> impl Iterator<Item = Transition> + '_ {
        self.support.iter().map(|(transition, _)| *transition)
    }
}

impl<T: TokenOps> FromIterator<(Transition, T)> for ParikhVector<T> {
    fn from_iter<I: IntoIterator<Item = (Transition, T)>>(iter: I) -> Self {
        let mut vec: Vec<(Transition, T)> = iter.into_iter().filter(|(_, t)| *t != T::ZERO).collect();
        vec.sort_unstable_by_key(|elem| elem.0.0);
        vec.dedup_by_key(|elem| elem.0.0);
        ParikhVector { support: vec }
    }
}

impl<T: TokenOps, const N: usize> From<[(Transition, T); N]> for ParikhVector<T> {
    fn from(array: [(Transition, T); N]) -> Self {
        ParikhVector::from_iter(array)
    }
}

impl<T: TokenOps> ParikhVector<T> {
    /// Returns the number of tokens this Parikh vector assigns to the provided [`Transition`].
    #[must_use]
    pub fn get(&self, transition: Transition) -> T {
        self.support
            .iter()
            .find(|(t, _)| *t == transition)
            .map_or(T::ZERO, |(_, t)| *t)
    }

    pub fn total_firings(&self) -> T {
        self.support.iter().map(|(_, t)| *t).sum()
    }
}

impl<T> IntoIterator for ParikhVector<T> {
    type Item = (Transition, T);
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.support.into_iter()
    }
}
