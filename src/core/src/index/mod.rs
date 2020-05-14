//! # Indexing structures for fast similarity search
//!
//! An index organizes signatures to allow for fast similarity search.
//! Some indices also support containment searches.

pub mod bigsi;
pub mod linear;
pub mod sbt;

pub mod storage;

pub mod search;

use std::ops::Deref;
use std::path::Path;
use std::rc::Rc;

use failure::Error;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

use crate::index::sbt::{Node, SBT};
use crate::index::search::{search_minhashes, search_minhashes_containment};
use crate::index::storage::{ReadData, ReadDataError, Storage};
use crate::signature::Signature;
use crate::sketch::nodegraph::Nodegraph;

pub type MHBT = SBT<Node<Nodegraph>, Signature>;

/* FIXME: bring back after MQF works on macOS and Windows
use cfg_if::cfg_if;
cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
      use mqf::MQF;
      pub type MHMT = SBT<Node<MQF>, Signature>;
    }
}
*/

pub trait Index<'a> {
    type Item: Comparable<Self::Item>;
    //type SignatureIterator: Iterator<Item = Self::Item>;

    fn find<F>(
        &self,
        search_fn: F,
        sig: &Self::Item,
        threshold: f64,
    ) -> Result<Vec<&Self::Item>, Error>
    where
        F: Fn(&dyn Comparable<Self::Item>, &Self::Item, f64) -> bool,
    {
        Ok(self
            .signature_refs()
            .into_iter()
            .flat_map(|node| {
                if search_fn(&node, sig, threshold) {
                    Some(node)
                } else {
                    None
                }
            })
            .collect())
    }

    fn search(
        &self,
        sig: &Self::Item,
        threshold: f64,
        containment: bool,
    ) -> Result<Vec<&Self::Item>, Error> {
        if containment {
            self.find(search_minhashes_containment, sig, threshold)
        } else {
            self.find(search_minhashes, sig, threshold)
        }
    }

    //fn gather(&self, sig: &Self::Item, threshold: f64) -> Result<Vec<&Self::Item>, Error>;

    fn insert(&mut self, node: Self::Item) -> Result<(), Error>;

    fn batch_insert(&mut self, nodes: Vec<Self::Item>) -> Result<(), Error> {
        for node in nodes {
            self.insert(node)?;
        }

        Ok(())
    }

    fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Error>;

    fn load<P: AsRef<Path>>(path: P) -> Result<(), Error>;

    fn signatures(&self) -> Vec<Self::Item>;

    fn signature_refs(&self) -> Vec<&Self::Item>;

    /*
    fn iter_signatures(&self) -> Self::SignatureIterator;
    */
}

// TODO: split into two traits, Similarity and Containment?
pub trait Comparable<O> {
    fn similarity(&self, other: &O) -> f64;
    fn containment(&self, other: &O) -> f64;
}

impl<'a, N, L> Comparable<L> for &'a N
where
    N: Comparable<L>,
{
    fn similarity(&self, other: &L) -> f64 {
        (*self).similarity(&other)
    }

    fn containment(&self, other: &L) -> f64 {
        (*self).containment(&other)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DatasetInfo {
    pub filename: String,
    pub name: String,
    pub metadata: String,
}

#[derive(TypedBuilder, Default, Clone)]
pub struct SigStore<T> {
    filename: String,
    name: String,
    metadata: String,

    storage: Option<Rc<dyn Storage>>,

    #[builder(default)]
    data: OnceCell<T>,
}

impl<T> SigStore<T> {
    pub fn name(&self) -> String {
        self.name.clone()
    }
}

impl<T> std::fmt::Debug for SigStore<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SigStore [filename: {}, name: {}, metadata: {}]",
            self.filename, self.name, self.metadata
        )
    }
}

impl ReadData<Signature> for SigStore<Signature> {
    fn data(&self) -> Result<&Signature, Error> {
        if let Some(sig) = self.data.get() {
            Ok(sig)
        } else if let Some(storage) = &self.storage {
            let sig = self.data.get_or_init(|| {
                let raw = storage.load(&self.filename).unwrap();
                let sigs: Result<Vec<Signature>, _> = serde_json::from_reader(&mut &raw[..]);
                if let Ok(sigs) = sigs {
                    // TODO: select the right sig?
                    sigs[0].to_owned()
                } else {
                    let sig: Signature = serde_json::from_reader(&mut &raw[..]).unwrap();
                    sig
                }
            });

            Ok(sig)
        } else {
            Err(ReadDataError::LoadError.into())
        }
    }
}

impl From<SigStore<Signature>> for Signature {
    fn from(other: SigStore<Signature>) -> Signature {
        other.data.get().unwrap().to_owned()
    }
}

impl Deref for SigStore<Signature> {
    type Target = Signature;

    fn deref(&self) -> &Signature {
        self.data.get().unwrap()
    }
}

impl From<Signature> for SigStore<Signature> {
    fn from(other: Signature) -> SigStore<Signature> {
        let name = other.name();
        let filename = other.filename();

        SigStore::builder()
            .name(name)
            .filename(filename)
            .data(other)
            .metadata("")
            .storage(None)
            .build()
    }
}

impl Comparable<SigStore<Signature>> for SigStore<Signature> {
    fn similarity(&self, other: &SigStore<Signature>) -> f64 {
        let ng: &Signature = self.data().unwrap();
        let ong: &Signature = other.data().unwrap();

        // TODO: select the right signatures...
        // TODO: better matching here, what if it is not a mh?
        let mh = &ng.signatures[0];
        let omh = &ong.signatures[0];

        mh.similarity(&omh).unwrap()
    }

    fn containment(&self, other: &SigStore<Signature>) -> f64 {
        let ng: &Signature = self.data().unwrap();
        let ong: &Signature = other.data().unwrap();

        // TODO: select the right signatures...
        // TODO: better matching here, what if it is not a mh?
        let mh = &ng.signatures[0];
        let omh = &ong.signatures[0];

        mh.containment(omh).unwrap()
    }
}

impl Comparable<Signature> for Signature {
    fn similarity(&self, other: &Signature) -> f64 {
        // TODO: select the right signatures...
        // TODO: better matching here, what if it is not a mh?
        let mh = &self.signatures[0];
        let omh = &other.signatures[0];
        return mh.similarity(&omh).unwrap();
    }

    fn containment(&self, other: &Signature) -> f64 {
        // TODO: select the right signatures...
        // TODO: better matching here, what if it is not a mh?
        let mh = &self.signatures[0];
        let omh = &other.signatures[0];

        mh.containment(omh).unwrap()
    }
}

impl<L> From<DatasetInfo> for SigStore<L> {
    fn from(other: DatasetInfo) -> SigStore<L> {
        SigStore {
            filename: other.filename,
            name: other.name,
            metadata: other.metadata,
            storage: None,
            data: OnceCell::new(),
        }
    }
}
