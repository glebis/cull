use crate::db_core::smart_collections::FilterNode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RejectedVisibility {
    Hide,
    Include,
}

impl RejectedVisibility {
    pub(crate) fn from_include_rejected(include_rejected: bool) -> Self {
        if include_rejected {
            Self::Include
        } else {
            Self::Hide
        }
    }

    pub(crate) fn sql_predicate(self) -> &'static str {
        match self {
            Self::Hide => "(s.decision IS NULL OR s.decision != 'reject')",
            Self::Include => "1=1",
        }
    }

    pub(crate) fn for_filter(self, filter: &FilterNode) -> Self {
        if self == Self::Include || filter.explicitly_requests_rejected() {
            Self::Include
        } else {
            Self::Hide
        }
    }
}
