use chronicle_protocol::protocol::{
    chronicle_committed, chronicle_contradicted, chronicle_identity_from_submission,
    chronicle_operations_from_submission, deserialize_submission, messages::Submission,
};
use common::{
    identity::{AuthId, OpaData, SignedIdentity},
    ledger::{OperationState, StateOutput, SubmissionError},
    opa::ExecutorContext,
    prov::{
        operations::ChronicleOperation, to_json_ld::ToJson, ChronicleTransaction,
        ChronicleTransactionId, ProcessorError, ProvModel,
    },
};
use prost::Message;
use std::collections::{BTreeMap, HashSet};

use chronicle_protocol::address::{SawtoothAddress, FAMILY, PREFIX, VERSION};

use sawtooth_sdk::{
    messages::processor::TpProcessRequest,
    processor::handler::{ApplyError, TransactionContext, TransactionHandler},
};
use tracing::{error, info, instrument, trace};

use crate::{
    abstract_tp::{TPSideEffects, TP},
    opa::TpOpa,
};

#[derive(Debug)]
pub struct ChronicleTransactionHandler {
    family_name: String,
    family_versions: Vec<String>,
    namespaces: Vec<String>,
    opa_executor: TpOpa,
}

impl ChronicleTransactionHandler {
    pub fn new(policy: &str, entrypoint: &str) -> Result<ChronicleTransactionHandler, ApplyError> {
        Ok(ChronicleTransactionHandler {
            family_name: FAMILY.to_owned(),
            family_versions: vec![VERSION.to_owned()],
            namespaces: vec![PREFIX.to_string()],
            opa_executor: TpOpa::new(policy, entrypoint)?,
        })
    }
}

#[async_trait::async_trait]
impl TP for ChronicleTransactionHandler {
    fn tp_parse(request: &TpProcessRequest) -> Result<Submission, ApplyError> {
        deserialize_submission(request.get_payload())
            .map_err(|e| ApplyError::InternalError(e.to_string()))
    }

    fn tp_state(
        context: &mut dyn TransactionContext,
        operations: &ChronicleTransaction,
    ) -> Result<OperationState<SawtoothAddress>, ApplyError> {
        let deps = operations
            .tx
            .iter()
            .flat_map(|tx| tx.dependencies())
            .collect::<HashSet<_>>();

        let addresses_to_load = deps.iter().map(SawtoothAddress::from).collect::<Vec<_>>();

        // Entries not present in state must be None
        let sawtooth_entries = context
            .get_state_entries(
                &addresses_to_load
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>(),
            )?
            .into_iter()
            .map(|(addr, data)| {
                (
                    SawtoothAddress::new(addr),
                    Some(String::from_utf8(data).unwrap()),
                )
            })
            .collect::<Vec<_>>();

        let mut state = OperationState::<SawtoothAddress>::new();

        let not_in_sawtooth = addresses_to_load
            .iter()
            .filter(|required_addr| {
                !sawtooth_entries
                    .iter()
                    .any(|(addr, _)| addr == *required_addr)
            })
            .map(|addr| (addr.clone(), None))
            .collect::<Vec<_>>();

        state.update_state(sawtooth_entries.into_iter());
        state.update_state(not_in_sawtooth.into_iter());

        Ok(state)
    }

    async fn tp_operations(submission: Submission) -> Result<ChronicleTransaction, ApplyError> {
        let identity = chronicle_identity_from_submission(submission.identity)
            .await
            .map_err(|e| ApplyError::InternalError(e.to_string()))?;
        let body = chronicle_operations_from_submission(submission.body)
            .await
            .map_err(|e| ApplyError::InternalError(e.to_string()))?;
        Ok(ChronicleTransaction::new(body, identity))
    }

    async fn tp(
        opa_executor: ExecutorContext,
        request: &TpProcessRequest,
        submission: Submission,
        operations: ChronicleTransaction,
        mut state: OperationState<SawtoothAddress>,
    ) -> Result<TPSideEffects, ApplyError> {
        let mut effects = TPSideEffects::new();

        let _protocol_version = submission.version;
        let span = submission.span_id;
        let id = request.get_signature().to_owned();

        info!(transaction_id = %id, span = %span, operation_count = %operations.tx.len(), identity = %submission.identity);

        //pre compute dependencies
        let deps = operations
            .tx
            .iter()
            .flat_map(|tx| tx.dependencies())
            .collect::<HashSet<_>>();

        let deps_as_sawtooth = deps
            .iter()
            .map(SawtoothAddress::from)
            .collect::<HashSet<_>>();

        trace!(
            input_chronicle_addresses=?deps,
        );

        let mut model = ProvModel::default();

        // Now apply operations to the model
        for operation in operations.tx {
            Self::enforce_opa(
                opa_executor.clone(),
                &operations.identity,
                &operation,
                &state,
            )
            .await?;

            let res = operation.process(model, state.input()).await;
            match res {
                // A contradiction raises an event and shortcuts processing
                Err(ProcessorError::Contradiction(source)) => {
                    info!(contradiction = %source);
                    let ev = chronicle_contradicted(span, &source, &operations.identity)
                        .map_err(|e| ApplyError::InternalError(e.to_string()))?;
                    effects.add_event(
                        "chronicle/prov-update".to_string(),
                        vec![("transaction_id".to_owned(), request.signature.clone())],
                        ev.encode_to_vec(),
                    );
                    return Ok(effects);
                }
                // Severe errors should be logged
                Err(e) => {
                    error!(chronicle_prov_failure = %e);

                    return Ok(effects);
                }
                Ok((tx_output, updated_model)) => {
                    state.update_state(
                        tx_output
                            .into_iter()
                            .map(|output| {
                                trace!(output_state = %output.data);
                                (SawtoothAddress::from(&output.address), Some(output.data))
                            })
                            .collect::<BTreeMap<_, _>>()
                            .into_iter(),
                    );
                    model = updated_model;
                }
            }
        }

        let dirty = state.dirty().collect::<Vec<_>>();

        trace!(dirty = ?dirty);

        let mut delta = ProvModel::default();
        for output in dirty
            .into_iter()
            .map(|output: StateOutput<SawtoothAddress>| {
                if deps_as_sawtooth.contains(&output.address) {
                    Ok(output)
                } else {
                    Err(SubmissionError::processor(
                        &ChronicleTransactionId::from(&*id),
                        ProcessorError::Address {},
                    ))
                }
            })
            .collect::<Result<Vec<_>, SubmissionError>>()
            .into_iter()
            .flat_map(|v: Vec<StateOutput<SawtoothAddress>>| v.into_iter())
        {
            let state: serde_json::Value = serde_json::from_str(&output.data)
                .map_err(|e| ApplyError::InternalError(e.to_string()))?;

            delta
                .apply_json_ld_str(&output.data)
                .await
                .map_err(|e| ApplyError::InternalError(e.to_string()))?;

            effects.set_state_entry(
                output.address.to_string(),
                serde_json::to_vec(&state).map_err(|e| ApplyError::InternalError(e.to_string()))?,
            )
        }

        // Finally emit the delta as an event
        let ev = chronicle_committed(span, delta, &operations.identity)
            .await
            .map_err(|e| ApplyError::InternalError(e.to_string()))?;

        effects.add_event(
            "chronicle/prov-update".to_string(),
            vec![("transaction_id".to_owned(), request.signature.clone())],
            ev.encode_to_vec(),
        );

        Ok(effects)
    }

    /// Get identity, the content of the compact json-ld representation of the operation, and a more
    /// readable serialization of the @graph from a provmodel containing the operation's dependencies and then
    /// pass them as the context to an OPA rule check, returning an error upon OPA policy failure
    async fn enforce_opa(
        opa_executor: ExecutorContext,
        identity: &SignedIdentity,
        tx: &ChronicleOperation,
        state: &OperationState<SawtoothAddress>,
    ) -> Result<(), ApplyError> {
        let identity = AuthId::try_from(identity)
            .map_err(|e| ApplyError::InternalError(ProcessorError::SerdeJson(e).to_string()))?;

        // Set up Context for OPA rule check
        let operation = tx
            .to_json()
            .compact()
            .await
            .map_err(|e| ApplyError::InternalError(ProcessorError::Compaction(e).to_string()))?
            .0;

        // Get the dependencies
        let deps = tx
            .dependencies()
            .into_iter()
            .collect::<HashSet<_>>()
            .iter()
            .map(SawtoothAddress::from)
            .collect::<HashSet<_>>();

        let operation_state = state.opa_context(deps);

        let state = serde_json::Value::Array(
            tx.opa_context_state(ProvModel::default(), operation_state)
                .await
                .map_err(|e| ApplyError::InternalError(e.to_string()))?,
        );

        let opa_data = OpaData::operation(&identity, &operation, &state);

        info!(opa_evaluation_context = ?opa_data);

        match opa_executor.evaluate(&identity, &opa_data).await {
            Ok(()) => Ok(()),
            Err(e) => Err(ApplyError::InvalidTransaction(e.to_string())),
        }
    }
}

impl TransactionHandler for ChronicleTransactionHandler {
    fn family_name(&self) -> String {
        self.family_name.clone()
    }

    fn family_versions(&self) -> Vec<String> {
        self.family_versions.clone()
    }

    fn namespaces(&self) -> Vec<String> {
        self.namespaces.clone()
    }

    #[instrument(
        name = "apply",
        level = "debug",
        skip(request,context),
        fields(
            transaction_id = %request.signature,
            inputs = ?request.header.as_ref().map(|x| &x.inputs),
            outputs = ?request.header.as_ref().map(|x| &x.outputs),
            dependencies = ?request.header.as_ref().map(|x| &x.dependencies)
        )
    )]
    fn apply(
        &self,
        request: &TpProcessRequest,
        context: &mut dyn TransactionContext,
    ) -> Result<(), ApplyError> {
        let submission = Self::tp_parse(request)?;
        let submission_clone = submission.clone();

        let opa_exec_context = self.opa_executor.executor_context(context)?;

        let operations =
            futures::executor::block_on(
                async move { Self::tp_operations(submission.clone()).await },
            )?;

        info!(transaction_id = %request.signature, operation_count = %operations.tx.len());

        let state = Self::tp_state(context, &operations)?;
        let effects = futures::executor::block_on(async move {
            Self::tp(
                opa_exec_context,
                request,
                submission_clone,
                operations,
                state,
            )
            .await
        })
        .map_err(|e| ApplyError::InternalError(e.to_string()))?;

        effects
            .apply(context)
            .map_err(|e| ApplyError::InternalError(e.to_string()))
    }
}

#[cfg(test)]
pub mod test {
    use std::{
        cell::RefCell,
        collections::{BTreeMap, BTreeSet},
    };

    use chronicle_protocol::{
        async_sawtooth_sdk::{ledger::LedgerTransaction, sawtooth::MessageBuilder},
        messages::ChronicleSubmitTransaction,
    };
    use common::{
        identity::AuthId,
        k256::ecdsa::SigningKey,
        prov::{
            operations::{ActsOnBehalfOf, AgentExists, ChronicleOperation, CreateNamespace},
            ActivityId, AgentId, ChronicleTransaction, DelegationId, ExternalId, ExternalIdPart,
            NamespaceId, Role,
        },
        signing::DirectoryStoredKeys,
    };
    use prost::Message;
    use sawtooth_sdk::{
        messages::{processor::TpProcessRequest, transaction::TransactionHeader},
        processor::handler::{ContextError, TransactionContext, TransactionHandler},
    };
    use serde_json::Value;
    use tempfile::TempDir;

    use uuid::Uuid;

    use crate::tp::ChronicleTransactionHandler;

    type TestTxEvents = Vec<(String, Vec<(String, String)>, Vec<u8>)>;

    pub struct TestTransactionContext {
        pub state: RefCell<BTreeMap<String, Vec<u8>>>,
        pub events: RefCell<TestTxEvents>,
    }

    type PrintableEvent = Vec<(String, Vec<(String, String)>, Value)>;

    impl TestTransactionContext {
        pub fn new() -> Self {
            Self {
                state: RefCell::new(BTreeMap::new()),
                events: RefCell::new(vec![]),
            }
        }

        pub fn readable_state(&self) -> Vec<(String, Value)> {
            self.state
                .borrow()
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        serde_json::from_str(&String::from_utf8(v.clone()).unwrap()).unwrap(),
                    )
                })
                .collect()
        }

        pub fn readable_events(&self) -> PrintableEvent {
            self.events
                .borrow()
                .iter()
                .map(|(k, attr, data)| {
                    (
                        k.clone(),
                        attr.clone(),
                        serde_json::from_str(
                            &chronicle_protocol::sawtooth::Event::decode(&**data)
                                .unwrap()
                                .delta,
                        )
                        .unwrap(),
                    )
                })
                .collect()
        }
    }

    impl Default for TestTransactionContext {
        fn default() -> Self {
            Self::new()
        }
    }

    impl TransactionContext for TestTransactionContext {
        fn add_receipt_data(
            self: &TestTransactionContext,
            _data: &[u8],
        ) -> Result<(), ContextError> {
            unimplemented!()
        }

        fn add_event(
            self: &TestTransactionContext,
            event_type: String,
            attributes: Vec<(String, String)>,
            data: &[u8],
        ) -> Result<(), ContextError> {
            self.events
                .borrow_mut()
                .push((event_type, attributes, data.to_vec()));
            Ok(())
        }

        fn delete_state_entries(
            self: &TestTransactionContext,
            _addresses: &[std::string::String],
        ) -> Result<Vec<String>, ContextError> {
            unimplemented!()
        }

        fn get_state_entries(
            &self,
            addresses: &[String],
        ) -> Result<Vec<(String, Vec<u8>)>, ContextError> {
            Ok(self
                .state
                .borrow()
                .iter()
                .filter(|(k, _)| addresses.contains(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect())
        }

        fn set_state_entries(
            self: &TestTransactionContext,
            entries: Vec<(String, Vec<u8>)>,
        ) -> std::result::Result<(), sawtooth_sdk::processor::handler::ContextError> {
            for entry in entries {
                self.state.borrow_mut().insert(entry.0, entry.1);
            }

            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    struct SameUuid;

    fn uuid() -> Uuid {
        Uuid::parse_str("5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea").unwrap()
    }

    fn create_namespace_id_helper(tag: Option<i32>) -> NamespaceId {
        let external_id = if tag.is_none() || tag == Some(0) {
            "testns".to_string()
        } else {
            format!("testns{}", tag.unwrap())
        };
        NamespaceId::from_external_id(external_id, uuid())
    }

    fn create_namespace_helper(tag: Option<i32>) -> ChronicleOperation {
        let id = create_namespace_id_helper(tag);
        let external_id = &id.external_id_part().to_string();
        ChronicleOperation::CreateNamespace(CreateNamespace::new(id, external_id, uuid()))
    }

    fn agent_exists_helper() -> ChronicleOperation {
        let namespace: NamespaceId = NamespaceId::from_external_id("testns", uuid());
        let external_id: ExternalId =
            ExternalIdPart::external_id_part(&AgentId::from_external_id("test_agent")).clone();
        ChronicleOperation::AgentExists(AgentExists {
            namespace,
            external_id,
        })
    }

    fn create_agent_acts_on_behalf_of() -> ChronicleOperation {
        let namespace: NamespaceId = NamespaceId::from_external_id("testns", uuid());
        let responsible_id = AgentId::from_external_id("test_agent");
        let delegate_id = AgentId::from_external_id("test_delegate");
        let activity_id = ActivityId::from_external_id("test_activity");
        let role = "test_role";
        let id = DelegationId::from_component_ids(
            &delegate_id,
            &responsible_id,
            Some(&activity_id),
            Some(role),
        );
        let role = Role::from(role.to_string());
        ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
            namespace,
            id,
            responsible_id,
            delegate_id,
            activity_id: Some(activity_id),
            role: Some(role),
        })
    }

    #[tokio::test]
    async fn simple_non_contradicting_operation() {
        chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);

        let keystore = DirectoryStoredKeys::new(TempDir::new().unwrap().into_path()).unwrap();
        keystore.generate_chronicle().unwrap();
        let signed_identity = AuthId::chronicle().signed_identity(&keystore).unwrap();

        // Example transaction payload of `CreateNamespace`,
        // `AgentExists`, and `AgentActsOnBehalfOf` `ChronicleOperation`s
        let tx = ChronicleTransaction::new(
            vec![
                create_namespace_helper(None),
                agent_exists_helper(),
                create_agent_acts_on_behalf_of(),
            ],
            signed_identity,
        );

        let secret: SigningKey = keystore.chronicle_signing().unwrap();

        let submit_tx = ChronicleSubmitTransaction {
            tx,
            signer: secret.clone(),
            policy_name: None,
        };

        let message_builder = MessageBuilder::new_deterministic("TEST", "1.0");
        // Get a signed tx from sawtooth protocol
        let (tx, _id) = submit_tx.as_sawtooth_tx(&message_builder).await;

        let header =
            <TransactionHeader as protobuf::Message>::parse_from_bytes(&tx.header).unwrap();

        let mut request = TpProcessRequest::default();
        request.set_header(header);
        request.set_payload(tx.payload);
        request.set_signature("TRANSACTION_SIGNATURE".to_string());

        let (policy, entrypoint) = ("allow_transactions", "allow_transactions.allowed_users");

        tokio::task::spawn_blocking(move || {
            // Create a `TestTransactionContext` to pass to the `tp` function
            let mut context = TestTransactionContext::new();
            let handler = ChronicleTransactionHandler::new(policy, entrypoint).unwrap();
            handler.apply(&request, &mut context).unwrap();

            insta::assert_yaml_snapshot!(context.readable_events(), @r###"
            ---
            - - chronicle/prov-update
              - - - transaction_id
                  - TRANSACTION_SIGNATURE
              - "@context": "https://btp.works/chr/1.0/c.jsonld"
                "@graph":
                  - "@id": "chronicle:activity:test%5Factivity"
                    "@type": "prov:Activity"
                    externalId: test_activity
                    namespace: "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea"
                    value: {}
                  - "@id": "chronicle:agent:test%5Fagent"
                    "@type": "prov:Agent"
                    externalId: test_agent
                    namespace: "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea"
                    value: {}
                  - "@id": "chronicle:agent:test%5Fdelegate"
                    "@type": "prov:Agent"
                    actedOnBehalfOf:
                      - "chronicle:agent:test%5Fagent"
                    externalId: test_delegate
                    namespace: "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea"
                    "prov:qualifiedDelegation":
                      "@id": "chronicle:delegation:test%5Fdelegate:test%5Fagent:role=test%5Frole:activity=test%5Factivity"
                    value: {}
                  - "@id": "chronicle:delegation:test%5Fdelegate:test%5Fagent:role=test%5Frole:activity=test%5Factivity"
                    "@type": "prov:Delegation"
                    actedOnBehalfOf:
                      - "chronicle:agent:test%5Fagent"
                    agent: "chronicle:agent:test%5Fdelegate"
                    namespace: "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea"
                    "prov:hadActivity":
                      "@id": "chronicle:activity:test%5Factivity"
                    "prov:hadRole": test_role
                  - "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea"
                    "@type": "chronicle:Namespace"
                    externalId: testns
            "###);
            insta::assert_yaml_snapshot!(context.readable_state(), @r###"
            ---
            - - 43a52b235b2c3e3735c87de6688c5e30596cd12fa3bc9d013c616035292f842fed5077
              - "@id": "chronicle:agent:test%5Fdelegate"
                "@type": "prov:Agent"
                actedOnBehalfOf:
                  - "chronicle:agent:test%5Fagent"
                externalId: test_delegate
                namespace: "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea"
                "prov:qualifiedDelegation":
                  "@id": "chronicle:delegation:test%5Fdelegate:test%5Fagent:role=test%5Frole:activity=test%5Factivity"
                value: {}
            - - 43a52b23e8079f2f7d6e21587c560d0d3e665c94adcbb3aed368b04eb73fcce3dc15a9
              - "@id": "chronicle:activity:test%5Factivity"
                "@type": "prov:Activity"
                externalId: test_activity
                namespace: "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea"
                value: {}
            - - 43a52b549abebadfd16401dc74e089fb79d0143453a060dc05453da719d0897097c08f
              - "@id": "chronicle:delegation:test%5Fdelegate:test%5Fagent:role=test%5Frole:activity=test%5Factivity"
                "@type": "prov:Delegation"
                actedOnBehalfOf:
                  - "chronicle:agent:test%5Fagent"
                agent: "chronicle:agent:test%5Fdelegate"
                namespace: "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea"
                "prov:hadActivity":
                  "@id": "chronicle:activity:test%5Factivity"
                "prov:hadRole": test_role
            - - 43a52be8b6d53163d3edd7e93e139a5f9adddb39e5481ee73a1b0326f26cf9abe90930
              - "@id": "chronicle:agent:test%5Fagent"
                "@type": "prov:Agent"
                externalId: test_agent
                namespace: "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea"
                value: {}
            - - 43a52bfdc37432b62f2f32862673fbbd3b7dbd1574c441fee886c5f80be47854c3a06e
              - "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea"
                "@type": "chronicle:Namespace"
                externalId: testns
            "###);
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn anonymous_access_denied() {
        chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);

        let keystore = DirectoryStoredKeys::new(TempDir::new().unwrap().into_path()).unwrap();
        keystore.generate_chronicle().unwrap();

        let user_identity = {
            let claims = common::identity::JwtClaims(
                serde_json::json!({
                    "sub": "abcdef",
                })
                .as_object()
                .unwrap()
                .to_owned(),
            );
            AuthId::from_jwt_claims(&claims, &BTreeSet::from(["sub".to_string()])).unwrap()
        };

        let signed_identity = user_identity.signed_identity(&keystore).unwrap();

        // Example transaction payload of `CreateNamespace`,
        // `AgentExists`, and `AgentActsOnBehalfOf` `ChronicleOperation`s
        let tx = ChronicleTransaction::new(
            vec![
                create_namespace_helper(None),
                agent_exists_helper(),
                create_agent_acts_on_behalf_of(),
            ],
            signed_identity,
        );

        let secret: SigningKey = keystore.chronicle_signing().unwrap();

        let submit_tx = ChronicleSubmitTransaction {
            tx,
            signer: secret.clone(),
            policy_name: None,
        };

        let message_builder = MessageBuilder::new_deterministic("TEST", "1.0");
        // Get a signed tx from sawtooth protocol
        let (tx, _id) = submit_tx.as_sawtooth_tx(&message_builder).await;

        let header =
            <TransactionHeader as protobuf::Message>::parse_from_bytes(&tx.header).unwrap();

        let mut request = TpProcessRequest::default();
        request.set_header(header);
        request.set_payload(tx.payload);
        request.set_signature("TRANSACTION_SIGNATURE".to_string());

        let (policy, entrypoint) = ("allow_transactions", "allow_transactions.allowed_users");

        tokio::task::spawn_blocking(move || {
            // Create a `TestTransactionContext` to pass to the `tp` function
            let mut context = TestTransactionContext::new();
            let handler = ChronicleTransactionHandler::new(policy, entrypoint).unwrap();
            match handler.apply(&request, &mut context) {
                Err(e) => {
                    insta::assert_snapshot!(e.to_string(), @"InternalError: InvalidTransaction: Access denied")
                }
                _ => panic!("expected error"),
            }
        })
        .await
        .unwrap();
    }
}
