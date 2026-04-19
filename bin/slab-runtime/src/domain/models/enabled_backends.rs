use std::collections::BTreeSet;

#[derive(Debug, Clone, Default)]
pub(crate) struct EnabledBackends {
    service_ids: BTreeSet<String>,
}

impl EnabledBackends {
    pub(crate) fn new<I, S>(service_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self { service_ids: service_ids.into_iter().map(Into::into).collect() }
    }

    pub(crate) fn contains(&self, service_id: &str) -> bool {
        self.service_ids.contains(service_id)
    }

    pub(crate) fn len(&self) -> usize {
        self.service_ids.len()
    }
}
