mod from_json_ld;
mod to_json_ld;

pub use from_json_ld::*;
pub use to_json_ld::*;

use iref::IriBuf;
use json_ld::NoLoader;
use lazy_static::lazy_static;
use locspan::Meta;
use rdf_types::{vocabulary::no_vocabulary_mut, BlankIdBuf};
use serde_json::Value;

#[cfg(feature = "std")]
use thiserror::Error;
#[cfg(not(feature = "std"))]
use thiserror_no_std::Error;

#[derive(Error, Debug)]
pub enum CompactionError {
	#[error("JSON-LD: {inner}")]
	JsonLd { inner: String },
	#[error("Tokio")]
	Join,
	#[error("Serde conversion: {source}")]
	Serde {
		#[from]
		#[source]
		source: serde_json::Error,
	},
	#[error("Expanded document invalid: {message}")]
	InvalidExpanded { message: String },
	#[error("Compacted document not a JSON object: {document}")]
	NoObject { document: Value },
}

#[derive(Debug)]
pub struct ExpandedJson(pub serde_json::Value);

fn construct_context_definition<M>(
	json: &serde_json::Value,
	metadata: M,
) -> json_ld::syntax::context::Definition<M>
where
	M: Clone + core::fmt::Debug,
{
	use json_ld::syntax::{
		context::{
			definition::{Bindings, Version},
			Definition, TermDefinition,
		},
		Entry, Nullable, TryFromJson,
	};
	if let Value::Object(map) = json {
		match map.get("@version") {
			None => {},
			Some(Value::Number(version)) if version.as_f64() == Some(1.1) => {},
			Some(json_version) => panic!("unexpected JSON-LD context @version: {json_version}"),
		};
		let mut bindings = Bindings::new();
		for (key, value) in map {
			if key == "@version" {
				// already handled above
			} else if let Some('@') = key.chars().next() {
				panic!("unexpected JSON-LD context key: {key}");
			} else {
				let value =
					json_ld::syntax::Value::from_serde_json(value.clone(), |_| metadata.clone());
				let term: Meta<TermDefinition<M>, M> = TryFromJson::try_from_json(value)
					.expect("failed to convert {value} to term binding");
				bindings.insert(
					Meta(key.clone().into(), metadata.clone()),
					Meta(Nullable::Some(term.value().clone()), metadata.clone()),
				);
			}
		}
		Definition {
			base: None,
			import: None,
			language: None,
			direction: None,
			propagate: None,
			protected: None,
			type_: None,
			version: Some(Entry::new(metadata.clone(), Meta::new(Version::V1_1, metadata))),
			vocab: None,
			bindings,
		}
	} else {
		panic!("failed to convert JSON to LD context: {json:?}");
	}
}

lazy_static! {
	static ref JSON_LD_CONTEXT_DEFS: json_ld::syntax::context::Definition<()> =
		construct_context_definition(&crate::context::PROV, ());
}

impl ExpandedJson {
	async fn compact_unordered(self) -> Result<CompactedJson, CompactionError> {
		use json_ld::{
			syntax::context, Compact, ExpandedDocument, Process, ProcessingMode, TryFromJson,
		};

		let vocabulary = no_vocabulary_mut();
		let mut loader: NoLoader<IriBuf, (), json_ld::syntax::Value> = NoLoader::new();

		// process context
		let value = context::Value::One(Meta::new(
			context::Context::Definition(JSON_LD_CONTEXT_DEFS.clone()),
			(),
		));
		let context_meta = Meta::new(value, ());
		let processed_context = context_meta
			.process(vocabulary, &mut loader, None)
			.await
			.map_err(|e| CompactionError::JsonLd { inner: format!("{:?}", e) })?;

		// compact document

		let expanded_meta = json_ld::syntax::Value::from_serde_json(self.0, |_| ());

		let expanded_doc: Meta<ExpandedDocument<IriBuf, BlankIdBuf, ()>, ()> =
			TryFromJson::try_from_json_in(vocabulary, expanded_meta).map_err(|e| {
				CompactionError::InvalidExpanded { message: format!("{:?}", e.into_value()) }
			})?;

		let output = expanded_doc
			.compact_full(
				vocabulary,
				processed_context.as_ref(),
				&mut loader,
				json_ld::compaction::Options {
					processing_mode: ProcessingMode::JsonLd1_1,
					compact_to_relative: true,
					compact_arrays: true,
					ordered: true,
				},
			)
			.await
			.map_err(|e| CompactionError::JsonLd { inner: e.to_string() })?;

		// Sort @graph

		// reference context
		let json: Value = output.into_value().into();

		if let Value::Object(mut map) = json {
			map.insert(
				"@context".to_string(),
				Value::String("http://chronicle.works/chr/1.0/c.jsonld".to_string()),
			);
			Ok(CompactedJson(Value::Object(map)))
		} else {
			Err(CompactionError::NoObject { document: json })
		}
	}

	// Sort @graph by json value, as they are unstable and we need deterministic output
	#[tracing::instrument(level = "trace", skip(self), ret)]
	pub async fn compact(self) -> Result<Value, CompactionError> {
		let mut v: serde_json::Value =
			serde_json::from_str(&self.compact_unordered().await?.0.to_string())?;

		if let Some(v) = v.pointer_mut("/@graph").and_then(|p| p.as_array_mut()) {
			v.sort_by_cached_key(|v| v.to_string());
		}

		Ok(v)
	}

	pub async fn compact_stable_order(self) -> Result<Value, CompactionError> {
		self.compact().await
	}
}

pub struct CompactedJson(pub serde_json::Value);

impl core::ops::Deref for CompactedJson {
	type Target = serde_json::Value;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl CompactedJson {
	pub fn pretty(&self) -> String {
		serde_json::to_string_pretty(&self.0).unwrap()
	}
}
