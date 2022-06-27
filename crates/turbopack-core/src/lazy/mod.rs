use std::{mem::replace, sync::Mutex};

use serde::{Deserialize, Serialize};
use turbo_tasks::{get_invalidator, trace::TraceRawVcs, Invalidator, Vc};
use turbo_tasks_fs::{FileContentVc, FileSystemPathVc};

use crate::{
    asset::{Asset, AssetVc},
    reference::AssetReferenceVc,
};

#[derive(Serialize, Deserialize, TraceRawVcs)]
enum LazyAssetState {
    Idle,
    Waiting(Invalidator),
    Expanded,
}

/// Asset decorator that only expands references of an asset when the content
/// has been read.
#[turbo_tasks::value(Asset, eq: manual)]
pub struct LazyAsset {
    asset: AssetVc,
    state: Mutex<LazyAssetState>,
}

impl PartialEq for LazyAsset {
    fn eq(&self, other: &Self) -> bool {
        self.asset == other.asset
    }
}

impl Eq for LazyAsset {}

#[turbo_tasks::value_impl]
impl LazyAssetVc {
    #[turbo_tasks::function]
    pub fn new(asset: AssetVc) -> Self {
        Self::slot(LazyAsset {
            asset,
            state: Mutex::new(LazyAssetState::Idle),
        })
    }
}

#[turbo_tasks::value_impl]
impl Asset for LazyAsset {
    #[turbo_tasks::function]
    fn path(&self) -> FileSystemPathVc {
        self.asset.path()
    }

    #[turbo_tasks::function]
    fn content(&self) -> FileContentVc {
        {
            let mut state = self.state.lock().unwrap();
            if let LazyAssetState::Waiting(invalidator) =
                replace(&mut *state, LazyAssetState::Expanded)
            {
                invalidator.invalidate();
            }
        }
        self.asset.content()
    }

    #[turbo_tasks::function]
    fn references(&self) -> Vc<Vec<AssetReferenceVc>> {
        let mut state = self.state.lock().unwrap();
        match &*state {
            LazyAssetState::Idle => {
                *state = LazyAssetState::Waiting(get_invalidator());
                Vc::slot(Vec::new())
            }
            LazyAssetState::Waiting(_) => unreachable!(),
            LazyAssetState::Expanded => self.asset.references(),
        }
    }
}