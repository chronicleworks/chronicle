#[allow(dead_code, unused_imports, non_camel_case_types)]
#[allow(clippy::all)]
#[allow(rustdoc::broken_intra_doc_links)]
pub mod api {
    #[allow(unused_imports)]
    mod root_mod {
        pub use super::*;
    }

    pub static PALLETS: [&str; 7usize] =
        ["System", "Timestamp", "Aura", "Grandpa", "Sudo", "Chronicle", "Opa"];
    pub static RUNTIME_APIS: [&str; 10usize] = [
        "ChronicleApi",
        "Core",
        "Metadata",
        "BlockBuilder",
        "TaggedTransactionQueue",
        "OffchainWorkerApi",
        "AuraApi",
        "SessionKeys",
        "GrandpaApi",
        "AccountNonceApi",
    ];

    #[doc = r" The error type returned when there is a runtime issue."]
    pub type DispatchError = runtime_types::sp_runtime::DispatchError;
    #[doc = r" The outer event enum."]
    pub type Event = runtime_types::runtime_chronicle::RuntimeEvent;
    #[doc = r" The outer extrinsic enum."]
    pub type Call = runtime_types::runtime_chronicle::RuntimeCall;
    #[doc = r" The outer error enum representing the DispatchError's Module variant."]
    pub type Error = runtime_types::runtime_chronicle::RuntimeError;

    pub fn constants() -> ConstantsApi {
        ConstantsApi
    }

    pub fn storage() -> StorageApi {
        StorageApi
    }

    pub fn tx() -> TransactionApi {
        TransactionApi
    }

    pub fn apis() -> runtime_apis::RuntimeApi {
        runtime_apis::RuntimeApi
    }

    pub mod runtime_apis {
        use super::{root_mod, runtime_types};
        use subxt::ext::codec::Encode;

        pub struct RuntimeApi;

        impl RuntimeApi {
            pub fn chronicle_api(&self) -> chronicle_api::ChronicleApi {
                chronicle_api::ChronicleApi
            }

            pub fn core(&self) -> core::Core {
                core::Core
            }

            pub fn metadata(&self) -> metadata::Metadata {
                metadata::Metadata
            }

            pub fn block_builder(&self) -> block_builder::BlockBuilder {
                block_builder::BlockBuilder
            }

            pub fn tagged_transaction_queue(
                &self,
            ) -> tagged_transaction_queue::TaggedTransactionQueue {
                tagged_transaction_queue::TaggedTransactionQueue
            }

            pub fn offchain_worker_api(&self) -> offchain_worker_api::OffchainWorkerApi {
                offchain_worker_api::OffchainWorkerApi
            }

            pub fn aura_api(&self) -> aura_api::AuraApi {
                aura_api::AuraApi
            }

            pub fn session_keys(&self) -> session_keys::SessionKeys {
                session_keys::SessionKeys
            }

            pub fn grandpa_api(&self) -> grandpa_api::GrandpaApi {
                grandpa_api::GrandpaApi
            }

            pub fn account_nonce_api(&self) -> account_nonce_api::AccountNonceApi {
                account_nonce_api::AccountNonceApi
            }
        }

        pub mod chronicle_api {
            use super::{root_mod, runtime_types};

            pub struct ChronicleApi;

            impl ChronicleApi {
                pub fn placeholder(
                    &self,
                ) -> ::subxt::runtime_api::Payload<
                    types::Placeholder,
                    types::placeholder::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "ChronicleApi",
                        "placeholder",
                        types::Placeholder {},
                        [
                            69u8, 86u8, 182u8, 109u8, 157u8, 7u8, 62u8, 57u8, 188u8, 29u8, 49u8,
                            204u8, 192u8, 72u8, 129u8, 172u8, 6u8, 99u8, 90u8, 91u8, 65u8, 63u8,
                            182u8, 117u8, 15u8, 156u8, 227u8, 205u8, 229u8, 70u8, 212u8, 119u8,
                        ],
                    )
                }
            }

            pub mod types {
                use super::runtime_types;

                pub mod placeholder {
                    use super::runtime_types;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ::core::primitive::u32;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct Placeholder {}
            }
        }

        pub mod core {
            use super::{root_mod, runtime_types};

            #[doc = " The `Core` runtime api that every Substrate runtime needs to implement."]
            pub struct Core;

            impl Core {
                #[doc = " Returns the version of the runtime."]
                pub fn version(
                    &self,
                ) -> ::subxt::runtime_api::Payload<types::Version, types::version::output::Output> {
                    ::subxt::runtime_api::Payload::new_static(
                        "Core",
                        "version",
                        types::Version {},
                        [
                            76u8, 202u8, 17u8, 117u8, 189u8, 237u8, 239u8, 237u8, 151u8, 17u8,
                            125u8, 159u8, 218u8, 92u8, 57u8, 238u8, 64u8, 147u8, 40u8, 72u8, 157u8,
                            116u8, 37u8, 195u8, 156u8, 27u8, 123u8, 173u8, 178u8, 102u8, 136u8,
                            6u8,
                        ],
                    )
                }

                #[doc = " Execute the given block."]
                pub fn execute_block(
                    &self,
                    block: types::execute_block::Block,
                ) -> ::subxt::runtime_api::Payload<
                    types::ExecuteBlock,
                    types::execute_block::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "Core",
                        "execute_block",
                        types::ExecuteBlock { block },
                        [
                            133u8, 135u8, 228u8, 65u8, 106u8, 27u8, 85u8, 158u8, 112u8, 254u8,
                            93u8, 26u8, 102u8, 201u8, 118u8, 216u8, 249u8, 247u8, 91u8, 74u8, 56u8,
                            208u8, 231u8, 115u8, 131u8, 29u8, 209u8, 6u8, 65u8, 57u8, 214u8, 125u8,
                        ],
                    )
                }

                #[doc = " Initialize a block with the given header."]
                pub fn initialize_block(
                    &self,
                    header: types::initialize_block::Header,
                ) -> ::subxt::runtime_api::Payload<
                    types::InitializeBlock,
                    types::initialize_block::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "Core",
                        "initialize_block",
                        types::InitializeBlock { header },
                        [
                            146u8, 138u8, 72u8, 240u8, 63u8, 96u8, 110u8, 189u8, 77u8, 92u8, 96u8,
                            232u8, 41u8, 217u8, 105u8, 148u8, 83u8, 190u8, 152u8, 219u8, 19u8,
                            87u8, 163u8, 1u8, 232u8, 25u8, 221u8, 74u8, 224u8, 67u8, 223u8, 34u8,
                        ],
                    )
                }
            }

            pub mod types {
                use super::runtime_types;

                pub mod version {
                    use super::runtime_types;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = runtime_types::sp_version::RuntimeVersion;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct Version {}

                pub mod execute_block {
                    use super::runtime_types;

                    pub type Block = runtime_types::sp_runtime::generic::block::Block<runtime_types::sp_runtime::generic::header::Header<::core::primitive::u32>, ::subxt::utils::UncheckedExtrinsic<::subxt::utils::MultiAddress<::subxt::utils::AccountId32, ()>, runtime_types::runtime_chronicle::RuntimeCall, runtime_types::sp_runtime::MultiSignature, (runtime_types::frame_system::extensions::check_non_zero_sender::CheckNonZeroSender, runtime_types::frame_system::extensions::check_spec_version::CheckSpecVersion, runtime_types::frame_system::extensions::check_tx_version::CheckTxVersion, runtime_types::frame_system::extensions::check_genesis::CheckGenesis, runtime_types::frame_system::extensions::check_mortality::CheckMortality, runtime_types::runtime_chronicle::no_nonce_fees::CheckNonce, runtime_types::frame_system::extensions::check_weight::CheckWeight, )>>;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ();
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct ExecuteBlock {
                    pub block: execute_block::Block,
                }

                pub mod initialize_block {
                    use super::runtime_types;

                    pub type Header =
                    runtime_types::sp_runtime::generic::header::Header<::core::primitive::u32>;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ();
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct InitializeBlock {
                    pub header: initialize_block::Header,
                }
            }
        }

        pub mod metadata {
            use super::{root_mod, runtime_types};

            #[doc = " The `Metadata` api trait that returns metadata for the runtime."]
            pub struct Metadata;

            impl Metadata {
                #[doc = " Returns the metadata of a runtime."]
                pub fn metadata(
                    &self,
                ) -> ::subxt::runtime_api::Payload<types::Metadata, types::metadata::output::Output>
                {
                    ::subxt::runtime_api::Payload::new_static(
                        "Metadata",
                        "metadata",
                        types::Metadata {},
                        [
                            231u8, 24u8, 67u8, 152u8, 23u8, 26u8, 188u8, 82u8, 229u8, 6u8, 185u8,
                            27u8, 175u8, 68u8, 83u8, 122u8, 69u8, 89u8, 185u8, 74u8, 248u8, 87u8,
                            217u8, 124u8, 193u8, 252u8, 199u8, 186u8, 196u8, 179u8, 179u8, 96u8,
                        ],
                    )
                }

                #[doc = " Returns the metadata at a given version."]
                #[doc = ""]
                #[doc = " If the given `version` isn't supported, this will return `None`."]
                #[doc = " Use [`Self::metadata_versions`] to find out about supported metadata version of the runtime."]
                pub fn metadata_at_version(
                    &self,
                    version: types::metadata_at_version::Version,
                ) -> ::subxt::runtime_api::Payload<
                    types::MetadataAtVersion,
                    types::metadata_at_version::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "Metadata",
                        "metadata_at_version",
                        types::MetadataAtVersion { version },
                        [
                            131u8, 53u8, 212u8, 234u8, 16u8, 25u8, 120u8, 252u8, 153u8, 153u8,
                            216u8, 28u8, 54u8, 113u8, 52u8, 236u8, 146u8, 68u8, 142u8, 8u8, 10u8,
                            169u8, 131u8, 142u8, 204u8, 38u8, 48u8, 108u8, 134u8, 86u8, 226u8,
                            61u8,
                        ],
                    )
                }

                #[doc = " Returns the supported metadata versions."]
                #[doc = ""]
                #[doc = " This can be used to call `metadata_at_version`."]
                pub fn metadata_versions(
                    &self,
                ) -> ::subxt::runtime_api::Payload<
                    types::MetadataVersions,
                    types::metadata_versions::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "Metadata",
                        "metadata_versions",
                        types::MetadataVersions {},
                        [
                            23u8, 144u8, 137u8, 91u8, 188u8, 39u8, 231u8, 208u8, 252u8, 218u8,
                            224u8, 176u8, 77u8, 32u8, 130u8, 212u8, 223u8, 76u8, 100u8, 190u8,
                            82u8, 94u8, 190u8, 8u8, 82u8, 244u8, 225u8, 179u8, 85u8, 176u8, 56u8,
                            16u8,
                        ],
                    )
                }
            }

            pub mod types {
                use super::runtime_types;

                pub mod metadata {
                    use super::runtime_types;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = runtime_types::sp_core::OpaqueMetadata;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct Metadata {}

                pub mod metadata_at_version {
                    use super::runtime_types;

                    pub type Version = ::core::primitive::u32;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output =
                        ::core::option::Option<runtime_types::sp_core::OpaqueMetadata>;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct MetadataAtVersion {
                    pub version: metadata_at_version::Version,
                }

                pub mod metadata_versions {
                    use super::runtime_types;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ::std::vec::Vec<::core::primitive::u32>;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct MetadataVersions {}
            }
        }

        pub mod block_builder {
            use super::{root_mod, runtime_types};

            #[doc = " The `BlockBuilder` api trait that provides the required functionality for building a block."]
            pub struct BlockBuilder;

            impl BlockBuilder {
                #[doc = " Apply the given extrinsic."]
                #[doc = ""]
                #[doc = " Returns an inclusion outcome which specifies if this extrinsic is included in"]
                #[doc = " this block or not."]
                pub fn apply_extrinsic(
                    &self,
                    extrinsic: types::apply_extrinsic::Extrinsic,
                ) -> ::subxt::runtime_api::Payload<
                    types::ApplyExtrinsic,
                    types::apply_extrinsic::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "BlockBuilder",
                        "apply_extrinsic",
                        types::ApplyExtrinsic { extrinsic },
                        [
                            72u8, 54u8, 139u8, 3u8, 118u8, 136u8, 65u8, 47u8, 6u8, 105u8, 125u8,
                            223u8, 160u8, 29u8, 103u8, 74u8, 79u8, 149u8, 48u8, 90u8, 237u8, 2u8,
                            97u8, 201u8, 123u8, 34u8, 167u8, 37u8, 187u8, 35u8, 176u8, 97u8,
                        ],
                    )
                }

                #[doc = " Finish the current block."]
                pub fn finalize_block(
                    &self,
                ) -> ::subxt::runtime_api::Payload<
                    types::FinalizeBlock,
                    types::finalize_block::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "BlockBuilder",
                        "finalize_block",
                        types::FinalizeBlock {},
                        [
                            244u8, 207u8, 24u8, 33u8, 13u8, 69u8, 9u8, 249u8, 145u8, 143u8, 122u8,
                            96u8, 197u8, 55u8, 64u8, 111u8, 238u8, 224u8, 34u8, 201u8, 27u8, 146u8,
                            232u8, 99u8, 191u8, 30u8, 114u8, 16u8, 32u8, 220u8, 58u8, 62u8,
                        ],
                    )
                }

                #[doc = " Generate inherent extrinsics. The inherent data will vary from chain to chain."]
                pub fn inherent_extrinsics(
                    &self,
                    inherent: types::inherent_extrinsics::Inherent,
                ) -> ::subxt::runtime_api::Payload<
                    types::InherentExtrinsics,
                    types::inherent_extrinsics::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "BlockBuilder",
                        "inherent_extrinsics",
                        types::InherentExtrinsics { inherent },
                        [
                            254u8, 110u8, 245u8, 201u8, 250u8, 192u8, 27u8, 228u8, 151u8, 213u8,
                            166u8, 89u8, 94u8, 81u8, 189u8, 234u8, 64u8, 18u8, 245u8, 80u8, 29u8,
                            18u8, 140u8, 129u8, 113u8, 236u8, 135u8, 55u8, 79u8, 159u8, 175u8,
                            183u8,
                        ],
                    )
                }

                #[doc = " Check that the inherents are valid. The inherent data will vary from chain to chain."]
                pub fn check_inherents(
                    &self,
                    block: types::check_inherents::Block,
                    data: types::check_inherents::Data,
                ) -> ::subxt::runtime_api::Payload<
                    types::CheckInherents,
                    types::check_inherents::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "BlockBuilder",
                        "check_inherents",
                        types::CheckInherents { block, data },
                        [
                            153u8, 134u8, 1u8, 215u8, 139u8, 11u8, 53u8, 51u8, 210u8, 175u8, 197u8,
                            28u8, 38u8, 209u8, 175u8, 247u8, 142u8, 157u8, 50u8, 151u8, 164u8,
                            191u8, 181u8, 118u8, 80u8, 97u8, 160u8, 248u8, 110u8, 217u8, 181u8,
                            234u8,
                        ],
                    )
                }
            }

            pub mod types {
                use super::runtime_types;

                pub mod apply_extrinsic {
                    use super::runtime_types;

                    pub type Extrinsic = ::subxt::utils::UncheckedExtrinsic<::subxt::utils::MultiAddress<::subxt::utils::AccountId32, ()>, runtime_types::runtime_chronicle::RuntimeCall, runtime_types::sp_runtime::MultiSignature, (runtime_types::frame_system::extensions::check_non_zero_sender::CheckNonZeroSender, runtime_types::frame_system::extensions::check_spec_version::CheckSpecVersion, runtime_types::frame_system::extensions::check_tx_version::CheckTxVersion, runtime_types::frame_system::extensions::check_genesis::CheckGenesis, runtime_types::frame_system::extensions::check_mortality::CheckMortality, runtime_types::runtime_chronicle::no_nonce_fees::CheckNonce, runtime_types::frame_system::extensions::check_weight::CheckWeight, )>;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ::core::result::Result<::core::result::Result<(), runtime_types::sp_runtime::DispatchError>, runtime_types::sp_runtime::transaction_validity::TransactionValidityError>;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct ApplyExtrinsic {
                    pub extrinsic: apply_extrinsic::Extrinsic,
                }

                pub mod finalize_block {
                    use super::runtime_types;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = runtime_types::sp_runtime::generic::header::Header<
                            ::core::primitive::u32,
                        >;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct FinalizeBlock {}

                pub mod inherent_extrinsics {
                    use super::runtime_types;

                    pub type Inherent = runtime_types::sp_inherents::InherentData;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ::std::vec::Vec<::subxt::utils::UncheckedExtrinsic<::subxt::utils::MultiAddress<::subxt::utils::AccountId32, ()>, runtime_types::runtime_chronicle::RuntimeCall, runtime_types::sp_runtime::MultiSignature, (runtime_types::frame_system::extensions::check_non_zero_sender::CheckNonZeroSender, runtime_types::frame_system::extensions::check_spec_version::CheckSpecVersion, runtime_types::frame_system::extensions::check_tx_version::CheckTxVersion, runtime_types::frame_system::extensions::check_genesis::CheckGenesis, runtime_types::frame_system::extensions::check_mortality::CheckMortality, runtime_types::runtime_chronicle::no_nonce_fees::CheckNonce, runtime_types::frame_system::extensions::check_weight::CheckWeight, )>>;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct InherentExtrinsics {
                    pub inherent: inherent_extrinsics::Inherent,
                }

                pub mod check_inherents {
                    use super::runtime_types;

                    pub type Block = runtime_types::sp_runtime::generic::block::Block<runtime_types::sp_runtime::generic::header::Header<::core::primitive::u32>, ::subxt::utils::UncheckedExtrinsic<::subxt::utils::MultiAddress<::subxt::utils::AccountId32, ()>, runtime_types::runtime_chronicle::RuntimeCall, runtime_types::sp_runtime::MultiSignature, (runtime_types::frame_system::extensions::check_non_zero_sender::CheckNonZeroSender, runtime_types::frame_system::extensions::check_spec_version::CheckSpecVersion, runtime_types::frame_system::extensions::check_tx_version::CheckTxVersion, runtime_types::frame_system::extensions::check_genesis::CheckGenesis, runtime_types::frame_system::extensions::check_mortality::CheckMortality, runtime_types::runtime_chronicle::no_nonce_fees::CheckNonce, runtime_types::frame_system::extensions::check_weight::CheckWeight, )>>;
                    pub type Data = runtime_types::sp_inherents::InherentData;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = runtime_types::sp_inherents::CheckInherentsResult;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct CheckInherents {
                    pub block: check_inherents::Block,
                    pub data: check_inherents::Data,
                }
            }
        }

        pub mod tagged_transaction_queue {
            use super::{root_mod, runtime_types};

            #[doc = " The `TaggedTransactionQueue` api trait for interfering with the transaction queue."]
            pub struct TaggedTransactionQueue;

            impl TaggedTransactionQueue {
                #[doc = " Validate the transaction."]
                #[doc = ""]
                #[doc = " This method is invoked by the transaction pool to learn details about given transaction."]
                #[doc = " The implementation should make sure to verify the correctness of the transaction"]
                #[doc = " against current state. The given `block_hash` corresponds to the hash of the block"]
                #[doc = " that is used as current state."]
                #[doc = ""]
                #[doc = " Note that this call may be performed by the pool multiple times and transactions"]
                #[doc = " might be verified in any possible order."]
                pub fn validate_transaction(
                    &self,
                    source: types::validate_transaction::Source,
                    tx: types::validate_transaction::Tx,
                    block_hash: types::validate_transaction::BlockHash,
                ) -> ::subxt::runtime_api::Payload<
                    types::ValidateTransaction,
                    types::validate_transaction::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "TaggedTransactionQueue",
                        "validate_transaction",
                        types::ValidateTransaction { source, tx, block_hash },
                        [
                            196u8, 50u8, 90u8, 49u8, 109u8, 251u8, 200u8, 35u8, 23u8, 150u8, 140u8,
                            143u8, 232u8, 164u8, 133u8, 89u8, 32u8, 240u8, 115u8, 39u8, 95u8, 70u8,
                            162u8, 76u8, 122u8, 73u8, 151u8, 144u8, 234u8, 120u8, 100u8, 29u8,
                        ],
                    )
                }
            }

            pub mod types {
                use super::runtime_types;

                pub mod validate_transaction {
                    use super::runtime_types;

                    pub type Source =
                    runtime_types::sp_runtime::transaction_validity::TransactionSource;
                    pub type Tx = ::subxt::utils::UncheckedExtrinsic<::subxt::utils::MultiAddress<::subxt::utils::AccountId32, ()>, runtime_types::runtime_chronicle::RuntimeCall, runtime_types::sp_runtime::MultiSignature, (runtime_types::frame_system::extensions::check_non_zero_sender::CheckNonZeroSender, runtime_types::frame_system::extensions::check_spec_version::CheckSpecVersion, runtime_types::frame_system::extensions::check_tx_version::CheckTxVersion, runtime_types::frame_system::extensions::check_genesis::CheckGenesis, runtime_types::frame_system::extensions::check_mortality::CheckMortality, runtime_types::runtime_chronicle::no_nonce_fees::CheckNonce, runtime_types::frame_system::extensions::check_weight::CheckWeight, )>;
                    pub type BlockHash = ::subxt::utils::H256;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ::core::result::Result<runtime_types::sp_runtime::transaction_validity::ValidTransaction, runtime_types::sp_runtime::transaction_validity::TransactionValidityError>;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct ValidateTransaction {
                    pub source: validate_transaction::Source,
                    pub tx: validate_transaction::Tx,
                    pub block_hash: validate_transaction::BlockHash,
                }
            }
        }

        pub mod offchain_worker_api {
            use super::{root_mod, runtime_types};

            #[doc = " The offchain worker api."]
            pub struct OffchainWorkerApi;

            impl OffchainWorkerApi {
                #[doc = " Starts the off-chain task for given block header."]
                pub fn offchain_worker(
                    &self,
                    header: types::offchain_worker::Header,
                ) -> ::subxt::runtime_api::Payload<
                    types::OffchainWorker,
                    types::offchain_worker::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "OffchainWorkerApi",
                        "offchain_worker",
                        types::OffchainWorker { header },
                        [
                            10u8, 135u8, 19u8, 153u8, 33u8, 216u8, 18u8, 242u8, 33u8, 140u8, 4u8,
                            223u8, 200u8, 130u8, 103u8, 118u8, 137u8, 24u8, 19u8, 127u8, 161u8,
                            29u8, 184u8, 111u8, 222u8, 111u8, 253u8, 73u8, 45u8, 31u8, 79u8, 60u8,
                        ],
                    )
                }
            }

            pub mod types {
                use super::runtime_types;

                pub mod offchain_worker {
                    use super::runtime_types;

                    pub type Header =
                    runtime_types::sp_runtime::generic::header::Header<::core::primitive::u32>;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ();
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct OffchainWorker {
                    pub header: offchain_worker::Header,
                }
            }
        }

        pub mod aura_api {
            use super::{root_mod, runtime_types};

            #[doc = " API necessary for block authorship with aura."]
            pub struct AuraApi;

            impl AuraApi {
                #[doc = " Returns the slot duration for Aura."]
                #[doc = ""]
                #[doc = " Currently, only the value provided by this type at genesis will be used."]
                pub fn slot_duration(
                    &self,
                ) -> ::subxt::runtime_api::Payload<
                    types::SlotDuration,
                    types::slot_duration::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "AuraApi",
                        "slot_duration",
                        types::SlotDuration {},
                        [
                            233u8, 210u8, 132u8, 172u8, 100u8, 125u8, 239u8, 92u8, 114u8, 82u8,
                            7u8, 110u8, 179u8, 196u8, 10u8, 19u8, 211u8, 15u8, 174u8, 2u8, 91u8,
                            73u8, 133u8, 100u8, 205u8, 201u8, 191u8, 60u8, 163u8, 122u8, 215u8,
                            10u8,
                        ],
                    )
                }

                #[doc = " Return the current set of authorities."]
                pub fn authorities(
                    &self,
                ) -> ::subxt::runtime_api::Payload<
                    types::Authorities,
                    types::authorities::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "AuraApi",
                        "authorities",
                        types::Authorities {},
                        [
                            96u8, 136u8, 226u8, 244u8, 105u8, 189u8, 8u8, 250u8, 71u8, 230u8, 37u8,
                            123u8, 218u8, 47u8, 179u8, 16u8, 170u8, 181u8, 165u8, 77u8, 102u8,
                            51u8, 43u8, 51u8, 186u8, 84u8, 49u8, 15u8, 208u8, 226u8, 129u8, 230u8,
                        ],
                    )
                }
            }

            pub mod types {
                use super::runtime_types;

                pub mod slot_duration {
                    use super::runtime_types;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = runtime_types::sp_consensus_slots::SlotDuration;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct SlotDuration {}

                pub mod authorities {
                    use super::runtime_types;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ::std::vec::Vec<
                            runtime_types::sp_consensus_aura::sr25519::app_sr25519::Public,
                        >;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct Authorities {}
            }
        }

        pub mod session_keys {
            use super::{root_mod, runtime_types};

            #[doc = " Session keys runtime api."]
            pub struct SessionKeys;

            impl SessionKeys {
                #[doc = " Generate a set of session keys with optionally using the given seed."]
                #[doc = " The keys should be stored within the keystore exposed via runtime"]
                #[doc = " externalities."]
                #[doc = ""]
                #[doc = " The seed needs to be a valid `utf8` string."]
                #[doc = ""]
                #[doc = " Returns the concatenated SCALE encoded public keys."]
                pub fn generate_session_keys(
                    &self,
                    seed: types::generate_session_keys::Seed,
                ) -> ::subxt::runtime_api::Payload<
                    types::GenerateSessionKeys,
                    types::generate_session_keys::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "SessionKeys",
                        "generate_session_keys",
                        types::GenerateSessionKeys { seed },
                        [
                            96u8, 171u8, 164u8, 166u8, 175u8, 102u8, 101u8, 47u8, 133u8, 95u8,
                            102u8, 202u8, 83u8, 26u8, 238u8, 47u8, 126u8, 132u8, 22u8, 11u8, 33u8,
                            190u8, 175u8, 94u8, 58u8, 245u8, 46u8, 80u8, 195u8, 184u8, 107u8, 65u8,
                        ],
                    )
                }

                #[doc = " Decode the given public session keys."]
                #[doc = ""]
                #[doc = " Returns the list of public raw public keys + key type."]
                pub fn decode_session_keys(
                    &self,
                    encoded: types::decode_session_keys::Encoded,
                ) -> ::subxt::runtime_api::Payload<
                    types::DecodeSessionKeys,
                    types::decode_session_keys::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "SessionKeys",
                        "decode_session_keys",
                        types::DecodeSessionKeys { encoded },
                        [
                            57u8, 242u8, 18u8, 51u8, 132u8, 110u8, 238u8, 255u8, 39u8, 194u8, 8u8,
                            54u8, 198u8, 178u8, 75u8, 151u8, 148u8, 176u8, 144u8, 197u8, 87u8,
                            29u8, 179u8, 235u8, 176u8, 78u8, 252u8, 103u8, 72u8, 203u8, 151u8,
                            248u8,
                        ],
                    )
                }
            }

            pub mod types {
                use super::runtime_types;

                pub mod generate_session_keys {
                    use super::runtime_types;

                    pub type Seed = ::core::option::Option<::std::vec::Vec<::core::primitive::u8>>;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ::std::vec::Vec<::core::primitive::u8>;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct GenerateSessionKeys {
                    pub seed: generate_session_keys::Seed,
                }

                pub mod decode_session_keys {
                    use super::runtime_types;

                    pub type Encoded = ::std::vec::Vec<::core::primitive::u8>;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ::core::option::Option<
                            ::std::vec::Vec<(
                                ::std::vec::Vec<::core::primitive::u8>,
                                runtime_types::sp_core::crypto::KeyTypeId,
                            )>,
                        >;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct DecodeSessionKeys {
                    pub encoded: decode_session_keys::Encoded,
                }
            }
        }

        pub mod grandpa_api {
            use super::{root_mod, runtime_types};

            #[doc = " APIs for integrating the GRANDPA finality gadget into runtimes."]
            #[doc = " This should be implemented on the runtime side."]
            #[doc = ""]
            #[doc = " This is primarily used for negotiating authority-set changes for the"]
            #[doc = " gadget. GRANDPA uses a signaling model of changing authority sets:"]
            #[doc = " changes should be signaled with a delay of N blocks, and then automatically"]
            #[doc = " applied in the runtime after those N blocks have passed."]
            #[doc = ""]
            #[doc = " The consensus protocol will coordinate the handoff externally."]
            pub struct GrandpaApi;

            impl GrandpaApi {
                #[doc = " Get the current GRANDPA authorities and weights. This should not change except"]
                #[doc = " for when changes are scheduled and the corresponding delay has passed."]
                #[doc = ""]
                #[doc = " When called at block B, it will return the set of authorities that should be"]
                #[doc = " used to finalize descendants of this block (B+1, B+2, ...). The block B itself"]
                #[doc = " is finalized by the authorities from block B-1."]
                pub fn grandpa_authorities(
                    &self,
                ) -> ::subxt::runtime_api::Payload<
                    types::GrandpaAuthorities,
                    types::grandpa_authorities::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "GrandpaApi",
                        "grandpa_authorities",
                        types::GrandpaAuthorities {},
                        [
                            166u8, 76u8, 160u8, 101u8, 242u8, 145u8, 213u8, 10u8, 16u8, 130u8,
                            230u8, 196u8, 125u8, 152u8, 92u8, 143u8, 119u8, 223u8, 140u8, 189u8,
                            203u8, 95u8, 52u8, 105u8, 147u8, 107u8, 135u8, 228u8, 62u8, 178u8,
                            128u8, 33u8,
                        ],
                    )
                }

                #[doc = " Submits an unsigned extrinsic to report an equivocation. The caller"]
                #[doc = " must provide the equivocation proof and a key ownership proof"]
                #[doc = " (should be obtained using `generate_key_ownership_proof`). The"]
                #[doc = " extrinsic will be unsigned and should only be accepted for local"]
                #[doc = " authorship (not to be broadcast to the network). This method returns"]
                #[doc = " `None` when creation of the extrinsic fails, e.g. if equivocation"]
                #[doc = " reporting is disabled for the given runtime (i.e. this method is"]
                #[doc = " hardcoded to return `None`). Only useful in an offchain context."]
                pub fn submit_report_equivocation_unsigned_extrinsic(
                    &self,
                    equivocation_proof: types::submit_report_equivocation_unsigned_extrinsic::EquivocationProof,
                    key_owner_proof: types::submit_report_equivocation_unsigned_extrinsic::KeyOwnerProof,
                ) -> ::subxt::runtime_api::Payload<
                    types::SubmitReportEquivocationUnsignedExtrinsic,
                    types::submit_report_equivocation_unsigned_extrinsic::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "GrandpaApi",
                        "submit_report_equivocation_unsigned_extrinsic",
                        types::SubmitReportEquivocationUnsignedExtrinsic {
                            equivocation_proof,
                            key_owner_proof,
                        },
                        [
                            112u8, 94u8, 150u8, 250u8, 132u8, 127u8, 185u8, 24u8, 113u8, 62u8,
                            28u8, 171u8, 83u8, 9u8, 41u8, 228u8, 92u8, 137u8, 29u8, 190u8, 214u8,
                            232u8, 100u8, 66u8, 100u8, 168u8, 149u8, 122u8, 93u8, 17u8, 236u8,
                            104u8,
                        ],
                    )
                }

                #[doc = " Generates a proof of key ownership for the given authority in the"]
                #[doc = " given set. An example usage of this module is coupled with the"]
                #[doc = " session historical module to prove that a given authority key is"]
                #[doc = " tied to a given staking identity during a specific session. Proofs"]
                #[doc = " of key ownership are necessary for submitting equivocation reports."]
                #[doc = " NOTE: even though the API takes a `set_id` as parameter the current"]
                #[doc = " implementations ignore this parameter and instead rely on this"]
                #[doc = " method being called at the correct block height, i.e. any point at"]
                #[doc = " which the given set id is live on-chain. Future implementations will"]
                #[doc = " instead use indexed data through an offchain worker, not requiring"]
                #[doc = " older states to be available."]
                pub fn generate_key_ownership_proof(
                    &self,
                    set_id: types::generate_key_ownership_proof::SetId,
                    authority_id: types::generate_key_ownership_proof::AuthorityId,
                ) -> ::subxt::runtime_api::Payload<
                    types::GenerateKeyOwnershipProof,
                    types::generate_key_ownership_proof::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "GrandpaApi",
                        "generate_key_ownership_proof",
                        types::GenerateKeyOwnershipProof { set_id, authority_id },
                        [
                            40u8, 126u8, 113u8, 27u8, 245u8, 45u8, 123u8, 138u8, 12u8, 3u8, 125u8,
                            186u8, 151u8, 53u8, 186u8, 93u8, 13u8, 150u8, 163u8, 176u8, 206u8,
                            89u8, 244u8, 127u8, 182u8, 85u8, 203u8, 41u8, 101u8, 183u8, 209u8,
                            179u8,
                        ],
                    )
                }

                #[doc = " Get current GRANDPA authority set id."]
                pub fn current_set_id(
                    &self,
                ) -> ::subxt::runtime_api::Payload<
                    types::CurrentSetId,
                    types::current_set_id::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "GrandpaApi",
                        "current_set_id",
                        types::CurrentSetId {},
                        [
                            42u8, 230u8, 120u8, 211u8, 156u8, 245u8, 109u8, 86u8, 100u8, 146u8,
                            234u8, 205u8, 41u8, 183u8, 109u8, 42u8, 17u8, 33u8, 156u8, 25u8, 139u8,
                            84u8, 101u8, 75u8, 232u8, 198u8, 87u8, 136u8, 218u8, 233u8, 103u8,
                            156u8,
                        ],
                    )
                }
            }

            pub mod types {
                use super::runtime_types;

                pub mod grandpa_authorities {
                    use super::runtime_types;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ::std::vec::Vec<(
                            runtime_types::sp_consensus_grandpa::app::Public,
                            ::core::primitive::u64,
                        )>;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct GrandpaAuthorities {}

                pub mod submit_report_equivocation_unsigned_extrinsic {
                    use super::runtime_types;

                    pub type EquivocationProof =
                    runtime_types::sp_consensus_grandpa::EquivocationProof<
                        ::subxt::utils::H256,
                        ::core::primitive::u32,
                    >;
                    pub type KeyOwnerProof =
                    runtime_types::sp_consensus_grandpa::OpaqueKeyOwnershipProof;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ::core::option::Option<()>;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct SubmitReportEquivocationUnsignedExtrinsic {
                    pub equivocation_proof:
                    submit_report_equivocation_unsigned_extrinsic::EquivocationProof,
                    pub key_owner_proof:
                    submit_report_equivocation_unsigned_extrinsic::KeyOwnerProof,
                }

                pub mod generate_key_ownership_proof {
                    use super::runtime_types;

                    pub type SetId = ::core::primitive::u64;
                    pub type AuthorityId = runtime_types::sp_consensus_grandpa::app::Public;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ::core::option::Option<
                            runtime_types::sp_consensus_grandpa::OpaqueKeyOwnershipProof,
                        >;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct GenerateKeyOwnershipProof {
                    pub set_id: generate_key_ownership_proof::SetId,
                    pub authority_id: generate_key_ownership_proof::AuthorityId,
                }

                pub mod current_set_id {
                    use super::runtime_types;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ::core::primitive::u64;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct CurrentSetId {}
            }
        }

        pub mod account_nonce_api {
            use super::{root_mod, runtime_types};

            #[doc = " The API to query account nonce."]
            pub struct AccountNonceApi;

            impl AccountNonceApi {
                #[doc = " Get current account nonce of given `AccountId`."]
                pub fn account_nonce(
                    &self,
                    account: types::account_nonce::Account,
                ) -> ::subxt::runtime_api::Payload<
                    types::AccountNonce,
                    types::account_nonce::output::Output,
                > {
                    ::subxt::runtime_api::Payload::new_static(
                        "AccountNonceApi",
                        "account_nonce",
                        types::AccountNonce { account },
                        [
                            231u8, 82u8, 7u8, 227u8, 131u8, 2u8, 215u8, 252u8, 173u8, 82u8, 11u8,
                            103u8, 200u8, 25u8, 114u8, 116u8, 79u8, 229u8, 152u8, 150u8, 236u8,
                            37u8, 101u8, 26u8, 220u8, 146u8, 182u8, 101u8, 73u8, 55u8, 191u8,
                            171u8,
                        ],
                    )
                }
            }

            pub mod types {
                use super::runtime_types;

                pub mod account_nonce {
                    use super::runtime_types;

                    pub type Account = ::subxt::utils::AccountId32;

                    pub mod output {
                        use super::runtime_types;

                        pub type Output = ::core::primitive::u32;
                    }
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct AccountNonce {
                    pub account: account_nonce::Account,
                }
            }
        }
    }

    pub fn custom() -> CustomValuesApi {
        CustomValuesApi
    }

    pub struct CustomValuesApi;

    impl CustomValuesApi {}

    pub struct ConstantsApi;

    impl ConstantsApi {
        pub fn system(&self) -> system::constants::ConstantsApi {
            system::constants::ConstantsApi
        }

        pub fn timestamp(&self) -> timestamp::constants::ConstantsApi {
            timestamp::constants::ConstantsApi
        }

        pub fn grandpa(&self) -> grandpa::constants::ConstantsApi {
            grandpa::constants::ConstantsApi
        }
    }

    pub struct StorageApi;

    impl StorageApi {
        pub fn system(&self) -> system::storage::StorageApi {
            system::storage::StorageApi
        }

        pub fn timestamp(&self) -> timestamp::storage::StorageApi {
            timestamp::storage::StorageApi
        }

        pub fn aura(&self) -> aura::storage::StorageApi {
            aura::storage::StorageApi
        }

        pub fn grandpa(&self) -> grandpa::storage::StorageApi {
            grandpa::storage::StorageApi
        }

        pub fn sudo(&self) -> sudo::storage::StorageApi {
            sudo::storage::StorageApi
        }

        pub fn chronicle(&self) -> chronicle::storage::StorageApi {
            chronicle::storage::StorageApi
        }

        pub fn opa(&self) -> opa::storage::StorageApi {
            opa::storage::StorageApi
        }
    }

    pub struct TransactionApi;

    impl TransactionApi {
        pub fn system(&self) -> system::calls::TransactionApi {
            system::calls::TransactionApi
        }

        pub fn timestamp(&self) -> timestamp::calls::TransactionApi {
            timestamp::calls::TransactionApi
        }

        pub fn grandpa(&self) -> grandpa::calls::TransactionApi {
            grandpa::calls::TransactionApi
        }

        pub fn sudo(&self) -> sudo::calls::TransactionApi {
            sudo::calls::TransactionApi
        }

        pub fn chronicle(&self) -> chronicle::calls::TransactionApi {
            chronicle::calls::TransactionApi
        }

        pub fn opa(&self) -> opa::calls::TransactionApi {
            opa::calls::TransactionApi
        }
    }

    #[doc = r" check whether the metadata provided is aligned with this statically generated code."]
    pub fn is_codegen_valid_for(metadata: &::subxt::Metadata) -> bool {
        let runtime_metadata_hash = metadata
            .hasher()
            .only_these_pallets(&PALLETS)
            .only_these_runtime_apis(&RUNTIME_APIS)
            .hash();
        runtime_metadata_hash
            == [
            132u8, 151u8, 134u8, 46u8, 233u8, 247u8, 71u8, 77u8, 208u8, 250u8, 224u8, 194u8,
            87u8, 250u8, 180u8, 8u8, 171u8, 141u8, 155u8, 124u8, 69u8, 131u8, 176u8, 140u8,
            166u8, 22u8, 252u8, 16u8, 219u8, 185u8, 158u8, 56u8,
        ]
    }

    pub mod system {
        use super::{root_mod, runtime_types};

        #[doc = "Error for the System pallet"]
        pub type Error = runtime_types::frame_system::pallet::Error;
        #[doc = "Contains a variant per dispatchable extrinsic that this pallet has."]
        pub type Call = runtime_types::frame_system::pallet::Call;

        pub mod calls {
            use super::{root_mod, runtime_types};

            type DispatchError = runtime_types::sp_runtime::DispatchError;

            pub mod types {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::remark`]."]
                pub struct Remark {
                    pub remark: remark::Remark,
                }

                pub mod remark {
                    use super::runtime_types;

                    pub type Remark = ::std::vec::Vec<::core::primitive::u8>;
                }

                impl ::subxt::blocks::StaticExtrinsic for Remark {
                    const CALL: &'static str = "remark";
                    const PALLET: &'static str = "System";
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::set_heap_pages`]."]
                pub struct SetHeapPages {
                    pub pages: set_heap_pages::Pages,
                }

                pub mod set_heap_pages {
                    use super::runtime_types;

                    pub type Pages = ::core::primitive::u64;
                }

                impl ::subxt::blocks::StaticExtrinsic for SetHeapPages {
                    const CALL: &'static str = "set_heap_pages";
                    const PALLET: &'static str = "System";
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::set_code`]."]
                pub struct SetCode {
                    pub code: set_code::Code,
                }

                pub mod set_code {
                    use super::runtime_types;

                    pub type Code = ::std::vec::Vec<::core::primitive::u8>;
                }

                impl ::subxt::blocks::StaticExtrinsic for SetCode {
                    const CALL: &'static str = "set_code";
                    const PALLET: &'static str = "System";
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::set_code_without_checks`]."]
                pub struct SetCodeWithoutChecks {
                    pub code: set_code_without_checks::Code,
                }

                pub mod set_code_without_checks {
                    use super::runtime_types;

                    pub type Code = ::std::vec::Vec<::core::primitive::u8>;
                }

                impl ::subxt::blocks::StaticExtrinsic for SetCodeWithoutChecks {
                    const CALL: &'static str = "set_code_without_checks";
                    const PALLET: &'static str = "System";
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::set_storage`]."]
                pub struct SetStorage {
                    pub items: set_storage::Items,
                }

                pub mod set_storage {
                    use super::runtime_types;

                    pub type Items = ::std::vec::Vec<(
                        ::std::vec::Vec<::core::primitive::u8>,
                        ::std::vec::Vec<::core::primitive::u8>,
                    )>;
                }

                impl ::subxt::blocks::StaticExtrinsic for SetStorage {
                    const CALL: &'static str = "set_storage";
                    const PALLET: &'static str = "System";
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::kill_storage`]."]
                pub struct KillStorage {
                    pub keys: kill_storage::Keys,
                }

                pub mod kill_storage {
                    use super::runtime_types;

                    pub type Keys = ::std::vec::Vec<::std::vec::Vec<::core::primitive::u8>>;
                }

                impl ::subxt::blocks::StaticExtrinsic for KillStorage {
                    const CALL: &'static str = "kill_storage";
                    const PALLET: &'static str = "System";
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::kill_prefix`]."]
                pub struct KillPrefix {
                    pub prefix: kill_prefix::Prefix,
                    pub subkeys: kill_prefix::Subkeys,
                }

                pub mod kill_prefix {
                    use super::runtime_types;

                    pub type Prefix = ::std::vec::Vec<::core::primitive::u8>;
                    pub type Subkeys = ::core::primitive::u32;
                }

                impl ::subxt::blocks::StaticExtrinsic for KillPrefix {
                    const CALL: &'static str = "kill_prefix";
                    const PALLET: &'static str = "System";
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::remark_with_event`]."]
                pub struct RemarkWithEvent {
                    pub remark: remark_with_event::Remark,
                }

                pub mod remark_with_event {
                    use super::runtime_types;

                    pub type Remark = ::std::vec::Vec<::core::primitive::u8>;
                }

                impl ::subxt::blocks::StaticExtrinsic for RemarkWithEvent {
                    const CALL: &'static str = "remark_with_event";
                    const PALLET: &'static str = "System";
                }
            }

            pub struct TransactionApi;

            impl TransactionApi {
                #[doc = "See [`Pallet::remark`]."]
                pub fn remark(
                    &self,
                    remark: types::remark::Remark,
                ) -> ::subxt::tx::Payload<types::Remark> {
                    ::subxt::tx::Payload::new_static(
                        "System",
                        "remark",
                        types::Remark { remark },
                        [
                            43u8, 126u8, 180u8, 174u8, 141u8, 48u8, 52u8, 125u8, 166u8, 212u8,
                            216u8, 98u8, 100u8, 24u8, 132u8, 71u8, 101u8, 64u8, 246u8, 169u8, 33u8,
                            250u8, 147u8, 208u8, 2u8, 40u8, 129u8, 209u8, 232u8, 207u8, 207u8,
                            13u8,
                        ],
                    )
                }

                #[doc = "See [`Pallet::set_heap_pages`]."]
                pub fn set_heap_pages(
                    &self,
                    pages: types::set_heap_pages::Pages,
                ) -> ::subxt::tx::Payload<types::SetHeapPages> {
                    ::subxt::tx::Payload::new_static(
                        "System",
                        "set_heap_pages",
                        types::SetHeapPages { pages },
                        [
                            188u8, 191u8, 99u8, 216u8, 219u8, 109u8, 141u8, 50u8, 78u8, 235u8,
                            215u8, 242u8, 195u8, 24u8, 111u8, 76u8, 229u8, 64u8, 99u8, 225u8,
                            134u8, 121u8, 81u8, 209u8, 127u8, 223u8, 98u8, 215u8, 150u8, 70u8,
                            57u8, 147u8,
                        ],
                    )
                }

                #[doc = "See [`Pallet::set_code`]."]
                pub fn set_code(
                    &self,
                    code: types::set_code::Code,
                ) -> ::subxt::tx::Payload<types::SetCode> {
                    ::subxt::tx::Payload::new_static(
                        "System",
                        "set_code",
                        types::SetCode { code },
                        [
                            233u8, 248u8, 88u8, 245u8, 28u8, 65u8, 25u8, 169u8, 35u8, 237u8, 19u8,
                            203u8, 136u8, 160u8, 18u8, 3u8, 20u8, 197u8, 81u8, 169u8, 244u8, 188u8,
                            27u8, 147u8, 147u8, 236u8, 65u8, 25u8, 3u8, 143u8, 182u8, 22u8,
                        ],
                    )
                }

                #[doc = "See [`Pallet::set_code_without_checks`]."]
                pub fn set_code_without_checks(
                    &self,
                    code: types::set_code_without_checks::Code,
                ) -> ::subxt::tx::Payload<types::SetCodeWithoutChecks> {
                    ::subxt::tx::Payload::new_static(
                        "System",
                        "set_code_without_checks",
                        types::SetCodeWithoutChecks { code },
                        [
                            82u8, 212u8, 157u8, 44u8, 70u8, 0u8, 143u8, 15u8, 109u8, 109u8, 107u8,
                            157u8, 141u8, 42u8, 169u8, 11u8, 15u8, 186u8, 252u8, 138u8, 10u8,
                            147u8, 15u8, 178u8, 247u8, 229u8, 213u8, 98u8, 207u8, 231u8, 119u8,
                            115u8,
                        ],
                    )
                }

                #[doc = "See [`Pallet::set_storage`]."]
                pub fn set_storage(
                    &self,
                    items: types::set_storage::Items,
                ) -> ::subxt::tx::Payload<types::SetStorage> {
                    ::subxt::tx::Payload::new_static(
                        "System",
                        "set_storage",
                        types::SetStorage { items },
                        [
                            141u8, 216u8, 52u8, 222u8, 223u8, 136u8, 123u8, 181u8, 19u8, 75u8,
                            163u8, 102u8, 229u8, 189u8, 158u8, 142u8, 95u8, 235u8, 240u8, 49u8,
                            150u8, 76u8, 78u8, 137u8, 126u8, 88u8, 183u8, 88u8, 231u8, 146u8,
                            234u8, 43u8,
                        ],
                    )
                }

                #[doc = "See [`Pallet::kill_storage`]."]
                pub fn kill_storage(
                    &self,
                    keys: types::kill_storage::Keys,
                ) -> ::subxt::tx::Payload<types::KillStorage> {
                    ::subxt::tx::Payload::new_static(
                        "System",
                        "kill_storage",
                        types::KillStorage { keys },
                        [
                            73u8, 63u8, 196u8, 36u8, 144u8, 114u8, 34u8, 213u8, 108u8, 93u8, 209u8,
                            234u8, 153u8, 185u8, 33u8, 91u8, 187u8, 195u8, 223u8, 130u8, 58u8,
                            156u8, 63u8, 47u8, 228u8, 249u8, 216u8, 139u8, 143u8, 177u8, 41u8,
                            35u8,
                        ],
                    )
                }

                #[doc = "See [`Pallet::kill_prefix`]."]
                pub fn kill_prefix(
                    &self,
                    prefix: types::kill_prefix::Prefix,
                    subkeys: types::kill_prefix::Subkeys,
                ) -> ::subxt::tx::Payload<types::KillPrefix> {
                    ::subxt::tx::Payload::new_static(
                        "System",
                        "kill_prefix",
                        types::KillPrefix { prefix, subkeys },
                        [
                            184u8, 57u8, 139u8, 24u8, 208u8, 87u8, 108u8, 215u8, 198u8, 189u8,
                            175u8, 242u8, 167u8, 215u8, 97u8, 63u8, 110u8, 166u8, 238u8, 98u8,
                            67u8, 236u8, 111u8, 110u8, 234u8, 81u8, 102u8, 5u8, 182u8, 5u8, 214u8,
                            85u8,
                        ],
                    )
                }

                #[doc = "See [`Pallet::remark_with_event`]."]
                pub fn remark_with_event(
                    &self,
                    remark: types::remark_with_event::Remark,
                ) -> ::subxt::tx::Payload<types::RemarkWithEvent> {
                    ::subxt::tx::Payload::new_static(
                        "System",
                        "remark_with_event",
                        types::RemarkWithEvent { remark },
                        [
                            120u8, 120u8, 153u8, 92u8, 184u8, 85u8, 34u8, 2u8, 174u8, 206u8, 105u8,
                            228u8, 233u8, 130u8, 80u8, 246u8, 228u8, 59u8, 234u8, 240u8, 4u8, 49u8,
                            147u8, 170u8, 115u8, 91u8, 149u8, 200u8, 228u8, 181u8, 8u8, 154u8,
                        ],
                    )
                }
            }
        }

        #[doc = "Event for the System pallet."]
        pub type Event = runtime_types::frame_system::pallet::Event;

        pub mod events {
            use super::runtime_types;

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            #[doc = "An extrinsic completed successfully."]
            pub struct ExtrinsicSuccess {
                pub dispatch_info: extrinsic_success::DispatchInfo,
            }

            pub mod extrinsic_success {
                use super::runtime_types;

                pub type DispatchInfo = runtime_types::frame_support::dispatch::DispatchInfo;
            }

            impl ::subxt::events::StaticEvent for ExtrinsicSuccess {
                const EVENT: &'static str = "ExtrinsicSuccess";
                const PALLET: &'static str = "System";
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            #[doc = "An extrinsic failed."]
            pub struct ExtrinsicFailed {
                pub dispatch_error: extrinsic_failed::DispatchError,
                pub dispatch_info: extrinsic_failed::DispatchInfo,
            }

            pub mod extrinsic_failed {
                use super::runtime_types;

                pub type DispatchError = runtime_types::sp_runtime::DispatchError;
                pub type DispatchInfo = runtime_types::frame_support::dispatch::DispatchInfo;
            }

            impl ::subxt::events::StaticEvent for ExtrinsicFailed {
                const EVENT: &'static str = "ExtrinsicFailed";
                const PALLET: &'static str = "System";
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            #[doc = "`:code` was updated."]
            pub struct CodeUpdated;

            impl ::subxt::events::StaticEvent for CodeUpdated {
                const EVENT: &'static str = "CodeUpdated";
                const PALLET: &'static str = "System";
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            #[doc = "A new account was created."]
            pub struct NewAccount {
                pub account: new_account::Account,
            }

            pub mod new_account {
                use super::runtime_types;

                pub type Account = ::subxt::utils::AccountId32;
            }

            impl ::subxt::events::StaticEvent for NewAccount {
                const EVENT: &'static str = "NewAccount";
                const PALLET: &'static str = "System";
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            #[doc = "An account was reaped."]
            pub struct KilledAccount {
                pub account: killed_account::Account,
            }

            pub mod killed_account {
                use super::runtime_types;

                pub type Account = ::subxt::utils::AccountId32;
            }

            impl ::subxt::events::StaticEvent for KilledAccount {
                const EVENT: &'static str = "KilledAccount";
                const PALLET: &'static str = "System";
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            #[doc = "On on-chain remark happened."]
            pub struct Remarked {
                pub sender: remarked::Sender,
                pub hash: remarked::Hash,
            }

            pub mod remarked {
                use super::runtime_types;

                pub type Sender = ::subxt::utils::AccountId32;
                pub type Hash = ::subxt::utils::H256;
            }

            impl ::subxt::events::StaticEvent for Remarked {
                const EVENT: &'static str = "Remarked";
                const PALLET: &'static str = "System";
            }
        }

        pub mod storage {
            use super::runtime_types;

            pub mod types {
                use super::runtime_types;

                pub mod account {
                    use super::runtime_types;

                    pub type Account =
                    runtime_types::frame_system::AccountInfo<::core::primitive::u32, ()>;
                    pub type Param0 = ::subxt::utils::AccountId32;
                }

                pub mod extrinsic_count {
                    use super::runtime_types;

                    pub type ExtrinsicCount = ::core::primitive::u32;
                }

                pub mod block_weight {
                    use super::runtime_types;

                    pub type BlockWeight = runtime_types::frame_support::dispatch::PerDispatchClass<
                        runtime_types::sp_weights::weight_v2::Weight,
                    >;
                }

                pub mod all_extrinsics_len {
                    use super::runtime_types;

                    pub type AllExtrinsicsLen = ::core::primitive::u32;
                }

                pub mod block_hash {
                    use super::runtime_types;

                    pub type BlockHash = ::subxt::utils::H256;
                    pub type Param0 = ::core::primitive::u32;
                }

                pub mod extrinsic_data {
                    use super::runtime_types;

                    pub type ExtrinsicData = ::std::vec::Vec<::core::primitive::u8>;
                    pub type Param0 = ::core::primitive::u32;
                }

                pub mod number {
                    use super::runtime_types;

                    pub type Number = ::core::primitive::u32;
                }

                pub mod parent_hash {
                    use super::runtime_types;

                    pub type ParentHash = ::subxt::utils::H256;
                }

                pub mod digest {
                    use super::runtime_types;

                    pub type Digest = runtime_types::sp_runtime::generic::digest::Digest;
                }

                pub mod events {
                    use super::runtime_types;

                    pub type Events = ::std::vec::Vec<
                        runtime_types::frame_system::EventRecord<
                            runtime_types::runtime_chronicle::RuntimeEvent,
                            ::subxt::utils::H256,
                        >,
                    >;
                }

                pub mod event_count {
                    use super::runtime_types;

                    pub type EventCount = ::core::primitive::u32;
                }

                pub mod event_topics {
                    use super::runtime_types;

                    pub type EventTopics =
                    ::std::vec::Vec<(::core::primitive::u32, ::core::primitive::u32)>;
                    pub type Param0 = ::subxt::utils::H256;
                }

                pub mod last_runtime_upgrade {
                    use super::runtime_types;

                    pub type LastRuntimeUpgrade =
                    runtime_types::frame_system::LastRuntimeUpgradeInfo;
                }

                pub mod upgraded_to_u32_ref_count {
                    use super::runtime_types;

                    pub type UpgradedToU32RefCount = ::core::primitive::bool;
                }

                pub mod upgraded_to_triple_ref_count {
                    use super::runtime_types;

                    pub type UpgradedToTripleRefCount = ::core::primitive::bool;
                }

                pub mod execution_phase {
                    use super::runtime_types;

                    pub type ExecutionPhase = runtime_types::frame_system::Phase;
                }
            }

            pub struct StorageApi;

            impl StorageApi {
                #[doc = " The full account information for a particular account ID."]
                pub fn account_iter(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::account::Account,
                    (),
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "Account",
                        vec![],
                        [
                            207u8, 128u8, 217u8, 6u8, 244u8, 231u8, 113u8, 230u8, 246u8, 220u8,
                            226u8, 62u8, 206u8, 203u8, 104u8, 119u8, 181u8, 97u8, 211u8, 3u8,
                            157u8, 102u8, 196u8, 131u8, 51u8, 221u8, 41u8, 183u8, 108u8, 28u8,
                            247u8, 73u8,
                        ],
                    )
                }

                #[doc = " The full account information for a particular account ID."]
                pub fn account(
                    &self,
                    _0: impl ::std::borrow::Borrow<types::account::Param0>,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::account::Account,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "Account",
                        vec![::subxt::storage::address::make_static_storage_map_key(_0.borrow())],
                        [
                            207u8, 128u8, 217u8, 6u8, 244u8, 231u8, 113u8, 230u8, 246u8, 220u8,
                            226u8, 62u8, 206u8, 203u8, 104u8, 119u8, 181u8, 97u8, 211u8, 3u8,
                            157u8, 102u8, 196u8, 131u8, 51u8, 221u8, 41u8, 183u8, 108u8, 28u8,
                            247u8, 73u8,
                        ],
                    )
                }

                #[doc = " Total extrinsics count for the current block."]
                pub fn extrinsic_count(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::extrinsic_count::ExtrinsicCount,
                    ::subxt::storage::address::Yes,
                    (),
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "ExtrinsicCount",
                        vec![],
                        [
                            102u8, 76u8, 236u8, 42u8, 40u8, 231u8, 33u8, 222u8, 123u8, 147u8,
                            153u8, 148u8, 234u8, 203u8, 181u8, 119u8, 6u8, 187u8, 177u8, 199u8,
                            120u8, 47u8, 137u8, 254u8, 96u8, 100u8, 165u8, 182u8, 249u8, 230u8,
                            159u8, 79u8,
                        ],
                    )
                }

                #[doc = " The current weight for the block."]
                pub fn block_weight(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::block_weight::BlockWeight,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "BlockWeight",
                        vec![],
                        [
                            158u8, 46u8, 228u8, 89u8, 210u8, 214u8, 84u8, 154u8, 50u8, 68u8, 63u8,
                            62u8, 43u8, 42u8, 99u8, 27u8, 54u8, 42u8, 146u8, 44u8, 241u8, 216u8,
                            229u8, 30u8, 216u8, 255u8, 165u8, 238u8, 181u8, 130u8, 36u8, 102u8,
                        ],
                    )
                }

                #[doc = " Total length (in bytes) for all extrinsics put together, for the current block."]
                pub fn all_extrinsics_len(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::all_extrinsics_len::AllExtrinsicsLen,
                    ::subxt::storage::address::Yes,
                    (),
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "AllExtrinsicsLen",
                        vec![],
                        [
                            117u8, 86u8, 61u8, 243u8, 41u8, 51u8, 102u8, 214u8, 137u8, 100u8,
                            243u8, 185u8, 122u8, 174u8, 187u8, 117u8, 86u8, 189u8, 63u8, 135u8,
                            101u8, 218u8, 203u8, 201u8, 237u8, 254u8, 128u8, 183u8, 169u8, 221u8,
                            242u8, 65u8,
                        ],
                    )
                }

                #[doc = " Map of block numbers to block hashes."]
                pub fn block_hash_iter(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::block_hash::BlockHash,
                    (),
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "BlockHash",
                        vec![],
                        [
                            217u8, 32u8, 215u8, 253u8, 24u8, 182u8, 207u8, 178u8, 157u8, 24u8,
                            103u8, 100u8, 195u8, 165u8, 69u8, 152u8, 112u8, 181u8, 56u8, 192u8,
                            164u8, 16u8, 20u8, 222u8, 28u8, 214u8, 144u8, 142u8, 146u8, 69u8,
                            202u8, 118u8,
                        ],
                    )
                }

                #[doc = " Map of block numbers to block hashes."]
                pub fn block_hash(
                    &self,
                    _0: impl ::std::borrow::Borrow<types::block_hash::Param0>,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::block_hash::BlockHash,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "BlockHash",
                        vec![::subxt::storage::address::make_static_storage_map_key(_0.borrow())],
                        [
                            217u8, 32u8, 215u8, 253u8, 24u8, 182u8, 207u8, 178u8, 157u8, 24u8,
                            103u8, 100u8, 195u8, 165u8, 69u8, 152u8, 112u8, 181u8, 56u8, 192u8,
                            164u8, 16u8, 20u8, 222u8, 28u8, 214u8, 144u8, 142u8, 146u8, 69u8,
                            202u8, 118u8,
                        ],
                    )
                }

                #[doc = " Extrinsics data for the current block (maps an extrinsic's index to its data)."]
                pub fn extrinsic_data_iter(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::extrinsic_data::ExtrinsicData,
                    (),
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "ExtrinsicData",
                        vec![],
                        [
                            160u8, 180u8, 122u8, 18u8, 196u8, 26u8, 2u8, 37u8, 115u8, 232u8, 133u8,
                            220u8, 106u8, 245u8, 4u8, 129u8, 42u8, 84u8, 241u8, 45u8, 199u8, 179u8,
                            128u8, 61u8, 170u8, 137u8, 231u8, 156u8, 247u8, 57u8, 47u8, 38u8,
                        ],
                    )
                }

                #[doc = " Extrinsics data for the current block (maps an extrinsic's index to its data)."]
                pub fn extrinsic_data(
                    &self,
                    _0: impl ::std::borrow::Borrow<types::extrinsic_data::Param0>,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::extrinsic_data::ExtrinsicData,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "ExtrinsicData",
                        vec![::subxt::storage::address::make_static_storage_map_key(_0.borrow())],
                        [
                            160u8, 180u8, 122u8, 18u8, 196u8, 26u8, 2u8, 37u8, 115u8, 232u8, 133u8,
                            220u8, 106u8, 245u8, 4u8, 129u8, 42u8, 84u8, 241u8, 45u8, 199u8, 179u8,
                            128u8, 61u8, 170u8, 137u8, 231u8, 156u8, 247u8, 57u8, 47u8, 38u8,
                        ],
                    )
                }

                #[doc = " The current block number being processed. Set by `execute_block`."]
                pub fn number(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::number::Number,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "Number",
                        vec![],
                        [
                            30u8, 194u8, 177u8, 90u8, 194u8, 232u8, 46u8, 180u8, 85u8, 129u8, 14u8,
                            9u8, 8u8, 8u8, 23u8, 95u8, 230u8, 5u8, 13u8, 105u8, 125u8, 2u8, 22u8,
                            200u8, 78u8, 93u8, 115u8, 28u8, 150u8, 113u8, 48u8, 53u8,
                        ],
                    )
                }

                #[doc = " Hash of the previous block."]
                pub fn parent_hash(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::parent_hash::ParentHash,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "ParentHash",
                        vec![],
                        [
                            26u8, 130u8, 11u8, 216u8, 155u8, 71u8, 128u8, 170u8, 30u8, 153u8, 21u8,
                            192u8, 62u8, 93u8, 137u8, 80u8, 120u8, 81u8, 202u8, 94u8, 248u8, 125u8,
                            71u8, 82u8, 141u8, 229u8, 32u8, 56u8, 73u8, 50u8, 101u8, 78u8,
                        ],
                    )
                }

                #[doc = " Digest of the current block, also part of the block header."]
                pub fn digest(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::digest::Digest,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "Digest",
                        vec![],
                        [
                            61u8, 64u8, 237u8, 91u8, 145u8, 232u8, 17u8, 254u8, 181u8, 16u8, 234u8,
                            91u8, 51u8, 140u8, 254u8, 131u8, 98u8, 135u8, 21u8, 37u8, 251u8, 20u8,
                            58u8, 92u8, 123u8, 141u8, 14u8, 227u8, 146u8, 46u8, 222u8, 117u8,
                        ],
                    )
                }

                #[doc = " Events deposited for the current block."]
                #[doc = ""]
                #[doc = " NOTE: The item is unbound and should therefore never be read on chain."]
                #[doc = " It could otherwise inflate the PoV size of a block."]
                #[doc = ""]
                #[doc = " Events have a large in-memory size. Box the events to not go out-of-memory"]
                #[doc = " just in case someone still reads them from within the runtime."]
                pub fn events(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::events::Events,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "Events",
                        vec![],
                        [
                            47u8, 5u8, 76u8, 40u8, 21u8, 207u8, 254u8, 42u8, 181u8, 203u8, 152u8,
                            15u8, 76u8, 55u8, 70u8, 116u8, 60u8, 212u8, 81u8, 157u8, 220u8, 244u8,
                            168u8, 174u8, 57u8, 37u8, 145u8, 109u8, 39u8, 83u8, 134u8, 248u8,
                        ],
                    )
                }

                #[doc = " The number of events in the `Events<T>` list."]
                pub fn event_count(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::event_count::EventCount,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "EventCount",
                        vec![],
                        [
                            175u8, 24u8, 252u8, 184u8, 210u8, 167u8, 146u8, 143u8, 164u8, 80u8,
                            151u8, 205u8, 189u8, 189u8, 55u8, 220u8, 47u8, 101u8, 181u8, 33u8,
                            254u8, 131u8, 13u8, 143u8, 3u8, 244u8, 245u8, 45u8, 2u8, 210u8, 79u8,
                            133u8,
                        ],
                    )
                }

                #[doc = " Mapping between a topic (represented by T::Hash) and a vector of indexes"]
                #[doc = " of events in the `<Events<T>>` list."]
                #[doc = ""]
                #[doc = " All topic vectors have deterministic storage locations depending on the topic. This"]
                #[doc = " allows light-clients to leverage the changes trie storage tracking mechanism and"]
                #[doc = " in case of changes fetch the list of events of interest."]
                #[doc = ""]
                #[doc = " The value has the type `(BlockNumberFor<T>, EventIndex)` because if we used only just"]
                #[doc = " the `EventIndex` then in case if the topic has the same contents on the next block"]
                #[doc = " no notification will be triggered thus the event might be lost."]
                pub fn event_topics_iter(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::event_topics::EventTopics,
                    (),
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "EventTopics",
                        vec![],
                        [
                            40u8, 225u8, 14u8, 75u8, 44u8, 176u8, 76u8, 34u8, 143u8, 107u8, 69u8,
                            133u8, 114u8, 13u8, 172u8, 250u8, 141u8, 73u8, 12u8, 65u8, 217u8, 63u8,
                            120u8, 241u8, 48u8, 106u8, 143u8, 161u8, 128u8, 100u8, 166u8, 59u8,
                        ],
                    )
                }

                #[doc = " Mapping between a topic (represented by T::Hash) and a vector of indexes"]
                #[doc = " of events in the `<Events<T>>` list."]
                #[doc = ""]
                #[doc = " All topic vectors have deterministic storage locations depending on the topic. This"]
                #[doc = " allows light-clients to leverage the changes trie storage tracking mechanism and"]
                #[doc = " in case of changes fetch the list of events of interest."]
                #[doc = ""]
                #[doc = " The value has the type `(BlockNumberFor<T>, EventIndex)` because if we used only just"]
                #[doc = " the `EventIndex` then in case if the topic has the same contents on the next block"]
                #[doc = " no notification will be triggered thus the event might be lost."]
                pub fn event_topics(
                    &self,
                    _0: impl ::std::borrow::Borrow<types::event_topics::Param0>,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::event_topics::EventTopics,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "EventTopics",
                        vec![::subxt::storage::address::make_static_storage_map_key(_0.borrow())],
                        [
                            40u8, 225u8, 14u8, 75u8, 44u8, 176u8, 76u8, 34u8, 143u8, 107u8, 69u8,
                            133u8, 114u8, 13u8, 172u8, 250u8, 141u8, 73u8, 12u8, 65u8, 217u8, 63u8,
                            120u8, 241u8, 48u8, 106u8, 143u8, 161u8, 128u8, 100u8, 166u8, 59u8,
                        ],
                    )
                }

                #[doc = " Stores the `spec_version` and `spec_name` of when the last runtime upgrade happened."]
                pub fn last_runtime_upgrade(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::last_runtime_upgrade::LastRuntimeUpgrade,
                    ::subxt::storage::address::Yes,
                    (),
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "LastRuntimeUpgrade",
                        vec![],
                        [
                            137u8, 29u8, 175u8, 75u8, 197u8, 208u8, 91u8, 207u8, 156u8, 87u8,
                            148u8, 68u8, 91u8, 140u8, 22u8, 233u8, 1u8, 229u8, 56u8, 34u8, 40u8,
                            194u8, 253u8, 30u8, 163u8, 39u8, 54u8, 209u8, 13u8, 27u8, 139u8, 184u8,
                        ],
                    )
                }

                #[doc = " True if we have upgraded so that `type RefCount` is `u32`. False (default) if not."]
                pub fn upgraded_to_u32_ref_count(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::upgraded_to_u32_ref_count::UpgradedToU32RefCount,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "UpgradedToU32RefCount",
                        vec![],
                        [
                            229u8, 73u8, 9u8, 132u8, 186u8, 116u8, 151u8, 171u8, 145u8, 29u8, 34u8,
                            130u8, 52u8, 146u8, 124u8, 175u8, 79u8, 189u8, 147u8, 230u8, 234u8,
                            107u8, 124u8, 31u8, 2u8, 22u8, 86u8, 190u8, 4u8, 147u8, 50u8, 245u8,
                        ],
                    )
                }

                #[doc = " True if we have upgraded so that AccountInfo contains three types of `RefCount`. False"]
                #[doc = " (default) if not."]
                pub fn upgraded_to_triple_ref_count(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::upgraded_to_triple_ref_count::UpgradedToTripleRefCount,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "UpgradedToTripleRefCount",
                        vec![],
                        [
                            97u8, 66u8, 124u8, 243u8, 27u8, 167u8, 147u8, 81u8, 254u8, 201u8,
                            101u8, 24u8, 40u8, 231u8, 14u8, 179u8, 154u8, 163u8, 71u8, 81u8, 185u8,
                            167u8, 82u8, 254u8, 189u8, 3u8, 101u8, 207u8, 206u8, 194u8, 155u8,
                            151u8,
                        ],
                    )
                }

                #[doc = " The execution phase of the block."]
                pub fn execution_phase(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::execution_phase::ExecutionPhase,
                    ::subxt::storage::address::Yes,
                    (),
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "System",
                        "ExecutionPhase",
                        vec![],
                        [
                            191u8, 129u8, 100u8, 134u8, 126u8, 116u8, 154u8, 203u8, 220u8, 200u8,
                            0u8, 26u8, 161u8, 250u8, 133u8, 205u8, 146u8, 24u8, 5u8, 156u8, 158u8,
                            35u8, 36u8, 253u8, 52u8, 235u8, 86u8, 167u8, 35u8, 100u8, 119u8, 27u8,
                        ],
                    )
                }
            }
        }

        pub mod constants {
            use super::runtime_types;

            pub struct ConstantsApi;

            impl ConstantsApi {
                #[doc = " Block & extrinsics weights: base values and limits."]
                pub fn block_weights(
                    &self,
                ) -> ::subxt::constants::Address<runtime_types::frame_system::limits::BlockWeights>
                {
                    ::subxt::constants::Address::new_static(
                        "System",
                        "BlockWeights",
                        [
                            176u8, 124u8, 225u8, 136u8, 25u8, 73u8, 247u8, 33u8, 82u8, 206u8, 85u8,
                            190u8, 127u8, 102u8, 71u8, 11u8, 185u8, 8u8, 58u8, 0u8, 94u8, 55u8,
                            163u8, 177u8, 104u8, 59u8, 60u8, 136u8, 246u8, 116u8, 0u8, 239u8,
                        ],
                    )
                }

                #[doc = " The maximum length of a block (in bytes)."]
                pub fn block_length(
                    &self,
                ) -> ::subxt::constants::Address<runtime_types::frame_system::limits::BlockLength> {
                    ::subxt::constants::Address::new_static(
                        "System",
                        "BlockLength",
                        [
                            23u8, 242u8, 225u8, 39u8, 225u8, 67u8, 152u8, 41u8, 155u8, 104u8, 68u8,
                            229u8, 185u8, 133u8, 10u8, 143u8, 184u8, 152u8, 234u8, 44u8, 140u8,
                            96u8, 166u8, 235u8, 162u8, 160u8, 72u8, 7u8, 35u8, 194u8, 3u8, 37u8,
                        ],
                    )
                }

                #[doc = " Maximum number of block number to block hash mappings to keep (oldest pruned first)."]
                pub fn block_hash_count(
                    &self,
                ) -> ::subxt::constants::Address<::core::primitive::u32> {
                    ::subxt::constants::Address::new_static(
                        "System",
                        "BlockHashCount",
                        [
                            98u8, 252u8, 116u8, 72u8, 26u8, 180u8, 225u8, 83u8, 200u8, 157u8,
                            125u8, 151u8, 53u8, 76u8, 168u8, 26u8, 10u8, 9u8, 98u8, 68u8, 9u8,
                            178u8, 197u8, 113u8, 31u8, 79u8, 200u8, 90u8, 203u8, 100u8, 41u8,
                            145u8,
                        ],
                    )
                }

                #[doc = " The weight of runtime database operations the runtime can invoke."]
                pub fn db_weight(
                    &self,
                ) -> ::subxt::constants::Address<runtime_types::sp_weights::RuntimeDbWeight> {
                    ::subxt::constants::Address::new_static(
                        "System",
                        "DbWeight",
                        [
                            42u8, 43u8, 178u8, 142u8, 243u8, 203u8, 60u8, 173u8, 118u8, 111u8,
                            200u8, 170u8, 102u8, 70u8, 237u8, 187u8, 198u8, 120u8, 153u8, 232u8,
                            183u8, 76u8, 74u8, 10u8, 70u8, 243u8, 14u8, 218u8, 213u8, 126u8, 29u8,
                            177u8,
                        ],
                    )
                }

                #[doc = " Get the chain's current version."]
                pub fn version(
                    &self,
                ) -> ::subxt::constants::Address<runtime_types::sp_version::RuntimeVersion> {
                    ::subxt::constants::Address::new_static(
                        "System",
                        "Version",
                        [
                            219u8, 45u8, 162u8, 245u8, 177u8, 246u8, 48u8, 126u8, 191u8, 157u8,
                            228u8, 83u8, 111u8, 133u8, 183u8, 13u8, 148u8, 108u8, 92u8, 102u8,
                            72u8, 205u8, 74u8, 242u8, 233u8, 79u8, 20u8, 170u8, 72u8, 202u8, 158u8,
                            165u8,
                        ],
                    )
                }

                #[doc = " The designated SS58 prefix of this chain."]
                #[doc = ""]
                #[doc = " This replaces the \"ss58Format\" property declared in the chain spec. Reason is"]
                #[doc = " that the runtime should know about the prefix in order to make use of it as"]
                #[doc = " an identifier of the chain."]
                pub fn ss58_prefix(&self) -> ::subxt::constants::Address<::core::primitive::u16> {
                    ::subxt::constants::Address::new_static(
                        "System",
                        "SS58Prefix",
                        [
                            116u8, 33u8, 2u8, 170u8, 181u8, 147u8, 171u8, 169u8, 167u8, 227u8,
                            41u8, 144u8, 11u8, 236u8, 82u8, 100u8, 74u8, 60u8, 184u8, 72u8, 169u8,
                            90u8, 208u8, 135u8, 15u8, 117u8, 10u8, 123u8, 128u8, 193u8, 29u8, 70u8,
                        ],
                    )
                }
            }
        }
    }

    pub mod timestamp {
        use super::{root_mod, runtime_types};

        #[doc = "Contains a variant per dispatchable extrinsic that this pallet has."]
        pub type Call = runtime_types::pallet_timestamp::pallet::Call;

        pub mod calls {
            use super::{root_mod, runtime_types};

            type DispatchError = runtime_types::sp_runtime::DispatchError;

            pub mod types {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::set`]."]
                pub struct Set {
                    #[codec(compact)]
                    pub now: set::Now,
                }

                pub mod set {
                    use super::runtime_types;

                    pub type Now = ::core::primitive::u64;
                }

                impl ::subxt::blocks::StaticExtrinsic for Set {
                    const CALL: &'static str = "set";
                    const PALLET: &'static str = "Timestamp";
                }
            }

            pub struct TransactionApi;

            impl TransactionApi {
                #[doc = "See [`Pallet::set`]."]
                pub fn set(&self, now: types::set::Now) -> ::subxt::tx::Payload<types::Set> {
                    ::subxt::tx::Payload::new_static(
                        "Timestamp",
                        "set",
                        types::Set { now },
                        [
                            37u8, 95u8, 49u8, 218u8, 24u8, 22u8, 0u8, 95u8, 72u8, 35u8, 155u8,
                            199u8, 213u8, 54u8, 207u8, 22u8, 185u8, 193u8, 221u8, 70u8, 18u8,
                            200u8, 4u8, 231u8, 195u8, 173u8, 6u8, 122u8, 11u8, 203u8, 231u8, 227u8,
                        ],
                    )
                }
            }
        }

        pub mod storage {
            use super::runtime_types;

            pub mod types {
                use super::runtime_types;

                pub mod now {
                    use super::runtime_types;

                    pub type Now = ::core::primitive::u64;
                }

                pub mod did_update {
                    use super::runtime_types;

                    pub type DidUpdate = ::core::primitive::bool;
                }
            }

            pub struct StorageApi;

            impl StorageApi {
                #[doc = " The current time for the current block."]
                pub fn now(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::now::Now,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Timestamp",
                        "Now",
                        vec![],
                        [
                            44u8, 50u8, 80u8, 30u8, 195u8, 146u8, 123u8, 238u8, 8u8, 163u8, 187u8,
                            92u8, 61u8, 39u8, 51u8, 29u8, 173u8, 169u8, 217u8, 158u8, 85u8, 187u8,
                            141u8, 26u8, 12u8, 115u8, 51u8, 11u8, 200u8, 244u8, 138u8, 152u8,
                        ],
                    )
                }

                #[doc = " Whether the timestamp has been updated in this block."]
                #[doc = ""]
                #[doc = " This value is updated to `true` upon successful submission of a timestamp by a node."]
                #[doc = " It is then checked at the end of each block execution in the `on_finalize` hook."]
                pub fn did_update(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::did_update::DidUpdate,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Timestamp",
                        "DidUpdate",
                        vec![],
                        [
                            229u8, 175u8, 246u8, 102u8, 237u8, 158u8, 212u8, 229u8, 238u8, 214u8,
                            205u8, 160u8, 164u8, 252u8, 195u8, 75u8, 139u8, 110u8, 22u8, 34u8,
                            248u8, 204u8, 107u8, 46u8, 20u8, 200u8, 238u8, 167u8, 71u8, 41u8,
                            214u8, 140u8,
                        ],
                    )
                }
            }
        }

        pub mod constants {
            use super::runtime_types;

            pub struct ConstantsApi;

            impl ConstantsApi {
                #[doc = " The minimum period between blocks."]
                #[doc = ""]
                #[doc = " Be aware that this is different to the *expected* period that the block production"]
                #[doc = " apparatus provides. Your chosen consensus system will generally work with this to"]
                #[doc = " determine a sensible block time. For example, in the Aura pallet it will be double this"]
                #[doc = " period on default settings."]
                pub fn minimum_period(
                    &self,
                ) -> ::subxt::constants::Address<::core::primitive::u64> {
                    ::subxt::constants::Address::new_static(
                        "Timestamp",
                        "MinimumPeriod",
                        [
                            128u8, 214u8, 205u8, 242u8, 181u8, 142u8, 124u8, 231u8, 190u8, 146u8,
                            59u8, 226u8, 157u8, 101u8, 103u8, 117u8, 249u8, 65u8, 18u8, 191u8,
                            103u8, 119u8, 53u8, 85u8, 81u8, 96u8, 220u8, 42u8, 184u8, 239u8, 42u8,
                            246u8,
                        ],
                    )
                }
            }
        }
    }

    pub mod aura {
        use super::{root_mod, runtime_types};

        pub mod storage {
            use super::runtime_types;

            pub mod types {
                use super::runtime_types;

                pub mod authorities {
                    use super::runtime_types;

                    pub type Authorities =
                    runtime_types::bounded_collections::bounded_vec::BoundedVec<
                        runtime_types::sp_consensus_aura::sr25519::app_sr25519::Public,
                    >;
                }

                pub mod current_slot {
                    use super::runtime_types;

                    pub type CurrentSlot = runtime_types::sp_consensus_slots::Slot;
                }
            }

            pub struct StorageApi;

            impl StorageApi {
                #[doc = " The current authority set."]
                pub fn authorities(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::authorities::Authorities,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Aura",
                        "Authorities",
                        vec![],
                        [
                            232u8, 129u8, 167u8, 104u8, 47u8, 188u8, 238u8, 164u8, 6u8, 29u8,
                            129u8, 45u8, 64u8, 182u8, 194u8, 47u8, 0u8, 73u8, 63u8, 102u8, 204u8,
                            94u8, 111u8, 96u8, 137u8, 7u8, 141u8, 110u8, 180u8, 80u8, 228u8, 16u8,
                        ],
                    )
                }

                #[doc = " The current slot of this block."]
                #[doc = ""]
                #[doc = " This will be set in `on_initialize`."]
                pub fn current_slot(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::current_slot::CurrentSlot,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Aura",
                        "CurrentSlot",
                        vec![],
                        [
                            112u8, 199u8, 115u8, 248u8, 217u8, 242u8, 45u8, 231u8, 178u8, 53u8,
                            236u8, 167u8, 219u8, 238u8, 81u8, 243u8, 39u8, 140u8, 68u8, 19u8,
                            201u8, 169u8, 211u8, 133u8, 135u8, 213u8, 150u8, 105u8, 60u8, 252u8,
                            43u8, 57u8,
                        ],
                    )
                }
            }
        }
    }

    pub mod grandpa {
        use super::{root_mod, runtime_types};

        #[doc = "The `Error` enum of this pallet."]
        pub type Error = runtime_types::pallet_grandpa::pallet::Error;
        #[doc = "Contains a variant per dispatchable extrinsic that this pallet has."]
        pub type Call = runtime_types::pallet_grandpa::pallet::Call;

        pub mod calls {
            use super::{root_mod, runtime_types};

            type DispatchError = runtime_types::sp_runtime::DispatchError;

            pub mod types {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::report_equivocation`]."]
                pub struct ReportEquivocation {
                    pub equivocation_proof:
                    ::std::boxed::Box<report_equivocation::EquivocationProof>,
                    pub key_owner_proof: report_equivocation::KeyOwnerProof,
                }

                pub mod report_equivocation {
                    use super::runtime_types;

                    pub type EquivocationProof =
                    runtime_types::sp_consensus_grandpa::EquivocationProof<
                        ::subxt::utils::H256,
                        ::core::primitive::u32,
                    >;
                    pub type KeyOwnerProof = runtime_types::sp_core::Void;
                }

                impl ::subxt::blocks::StaticExtrinsic for ReportEquivocation {
                    const CALL: &'static str = "report_equivocation";
                    const PALLET: &'static str = "Grandpa";
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::report_equivocation_unsigned`]."]
                pub struct ReportEquivocationUnsigned {
                    pub equivocation_proof:
                    ::std::boxed::Box<report_equivocation_unsigned::EquivocationProof>,
                    pub key_owner_proof: report_equivocation_unsigned::KeyOwnerProof,
                }

                pub mod report_equivocation_unsigned {
                    use super::runtime_types;

                    pub type EquivocationProof =
                    runtime_types::sp_consensus_grandpa::EquivocationProof<
                        ::subxt::utils::H256,
                        ::core::primitive::u32,
                    >;
                    pub type KeyOwnerProof = runtime_types::sp_core::Void;
                }

                impl ::subxt::blocks::StaticExtrinsic for ReportEquivocationUnsigned {
                    const CALL: &'static str = "report_equivocation_unsigned";
                    const PALLET: &'static str = "Grandpa";
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::note_stalled`]."]
                pub struct NoteStalled {
                    pub delay: note_stalled::Delay,
                    pub best_finalized_block_number: note_stalled::BestFinalizedBlockNumber,
                }

                pub mod note_stalled {
                    use super::runtime_types;

                    pub type Delay = ::core::primitive::u32;
                    pub type BestFinalizedBlockNumber = ::core::primitive::u32;
                }

                impl ::subxt::blocks::StaticExtrinsic for NoteStalled {
                    const CALL: &'static str = "note_stalled";
                    const PALLET: &'static str = "Grandpa";
                }
            }

            pub struct TransactionApi;

            impl TransactionApi {
                #[doc = "See [`Pallet::report_equivocation`]."]
                pub fn report_equivocation(
                    &self,
                    equivocation_proof: types::report_equivocation::EquivocationProof,
                    key_owner_proof: types::report_equivocation::KeyOwnerProof,
                ) -> ::subxt::tx::Payload<types::ReportEquivocation> {
                    ::subxt::tx::Payload::new_static(
                        "Grandpa",
                        "report_equivocation",
                        types::ReportEquivocation {
                            equivocation_proof: ::std::boxed::Box::new(equivocation_proof),
                            key_owner_proof,
                        },
                        [
                            158u8, 70u8, 189u8, 51u8, 231u8, 191u8, 199u8, 33u8, 64u8, 156u8, 71u8,
                            243u8, 122u8, 199u8, 216u8, 10u8, 45u8, 73u8, 198u8, 141u8, 31u8,
                            209u8, 58u8, 164u8, 219u8, 124u8, 242u8, 26u8, 114u8, 52u8, 65u8,
                            106u8,
                        ],
                    )
                }

                #[doc = "See [`Pallet::report_equivocation_unsigned`]."]
                pub fn report_equivocation_unsigned(
                    &self,
                    equivocation_proof: types::report_equivocation_unsigned::EquivocationProof,
                    key_owner_proof: types::report_equivocation_unsigned::KeyOwnerProof,
                ) -> ::subxt::tx::Payload<types::ReportEquivocationUnsigned> {
                    ::subxt::tx::Payload::new_static(
                        "Grandpa",
                        "report_equivocation_unsigned",
                        types::ReportEquivocationUnsigned {
                            equivocation_proof: ::std::boxed::Box::new(equivocation_proof),
                            key_owner_proof,
                        },
                        [
                            53u8, 23u8, 255u8, 215u8, 105u8, 11u8, 67u8, 177u8, 234u8, 248u8,
                            183u8, 57u8, 230u8, 239u8, 54u8, 238u8, 115u8, 170u8, 153u8, 18u8,
                            55u8, 195u8, 85u8, 98u8, 109u8, 194u8, 57u8, 225u8, 139u8, 237u8,
                            171u8, 152u8,
                        ],
                    )
                }

                #[doc = "See [`Pallet::note_stalled`]."]
                pub fn note_stalled(
                    &self,
                    delay: types::note_stalled::Delay,
                    best_finalized_block_number: types::note_stalled::BestFinalizedBlockNumber,
                ) -> ::subxt::tx::Payload<types::NoteStalled> {
                    ::subxt::tx::Payload::new_static(
                        "Grandpa",
                        "note_stalled",
                        types::NoteStalled { delay, best_finalized_block_number },
                        [
                            158u8, 25u8, 64u8, 114u8, 131u8, 139u8, 227u8, 132u8, 42u8, 107u8,
                            40u8, 249u8, 18u8, 93u8, 254u8, 86u8, 37u8, 67u8, 250u8, 35u8, 241u8,
                            194u8, 209u8, 20u8, 39u8, 75u8, 186u8, 21u8, 48u8, 124u8, 151u8, 31u8,
                        ],
                    )
                }
            }
        }

        #[doc = "The `Event` enum of this pallet"]
        pub type Event = runtime_types::pallet_grandpa::pallet::Event;

        pub mod events {
            use super::runtime_types;

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            #[doc = "New authority set has been applied."]
            pub struct NewAuthorities {
                pub authority_set: new_authorities::AuthoritySet,
            }

            pub mod new_authorities {
                use super::runtime_types;

                pub type AuthoritySet = ::std::vec::Vec<(
                    runtime_types::sp_consensus_grandpa::app::Public,
                    ::core::primitive::u64,
                )>;
            }

            impl ::subxt::events::StaticEvent for NewAuthorities {
                const EVENT: &'static str = "NewAuthorities";
                const PALLET: &'static str = "Grandpa";
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            #[doc = "Current authority set has been paused."]
            pub struct Paused;

            impl ::subxt::events::StaticEvent for Paused {
                const EVENT: &'static str = "Paused";
                const PALLET: &'static str = "Grandpa";
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            #[doc = "Current authority set has been resumed."]
            pub struct Resumed;

            impl ::subxt::events::StaticEvent for Resumed {
                const EVENT: &'static str = "Resumed";
                const PALLET: &'static str = "Grandpa";
            }
        }

        pub mod storage {
            use super::runtime_types;

            pub mod types {
                use super::runtime_types;

                pub mod state {
                    use super::runtime_types;

                    pub type State =
                    runtime_types::pallet_grandpa::StoredState<::core::primitive::u32>;
                }

                pub mod pending_change {
                    use super::runtime_types;

                    pub type PendingChange =
                    runtime_types::pallet_grandpa::StoredPendingChange<::core::primitive::u32>;
                }

                pub mod next_forced {
                    use super::runtime_types;

                    pub type NextForced = ::core::primitive::u32;
                }

                pub mod stalled {
                    use super::runtime_types;

                    pub type Stalled = (::core::primitive::u32, ::core::primitive::u32);
                }

                pub mod current_set_id {
                    use super::runtime_types;

                    pub type CurrentSetId = ::core::primitive::u64;
                }

                pub mod set_id_session {
                    use super::runtime_types;

                    pub type SetIdSession = ::core::primitive::u32;
                    pub type Param0 = ::core::primitive::u64;
                }
            }

            pub struct StorageApi;

            impl StorageApi {
                #[doc = " State of the current authority set."]
                pub fn state(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::state::State,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Grandpa",
                        "State",
                        vec![],
                        [
                            73u8, 71u8, 112u8, 83u8, 238u8, 75u8, 44u8, 9u8, 180u8, 33u8, 30u8,
                            121u8, 98u8, 96u8, 61u8, 133u8, 16u8, 70u8, 30u8, 249u8, 34u8, 148u8,
                            15u8, 239u8, 164u8, 157u8, 52u8, 27u8, 144u8, 52u8, 223u8, 109u8,
                        ],
                    )
                }

                #[doc = " Pending change: (signaled at, scheduled change)."]
                pub fn pending_change(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::pending_change::PendingChange,
                    ::subxt::storage::address::Yes,
                    (),
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Grandpa",
                        "PendingChange",
                        vec![],
                        [
                            150u8, 194u8, 185u8, 248u8, 239u8, 43u8, 141u8, 253u8, 61u8, 106u8,
                            74u8, 164u8, 209u8, 204u8, 206u8, 200u8, 32u8, 38u8, 11u8, 78u8, 84u8,
                            243u8, 181u8, 142u8, 179u8, 151u8, 81u8, 204u8, 244u8, 150u8, 137u8,
                            250u8,
                        ],
                    )
                }

                #[doc = " next block number where we can force a change."]
                pub fn next_forced(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::next_forced::NextForced,
                    ::subxt::storage::address::Yes,
                    (),
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Grandpa",
                        "NextForced",
                        vec![],
                        [
                            3u8, 231u8, 56u8, 18u8, 87u8, 112u8, 227u8, 126u8, 180u8, 131u8, 255u8,
                            141u8, 82u8, 34u8, 61u8, 47u8, 234u8, 37u8, 95u8, 62u8, 33u8, 235u8,
                            231u8, 122u8, 125u8, 8u8, 223u8, 95u8, 255u8, 204u8, 40u8, 97u8,
                        ],
                    )
                }

                #[doc = " `true` if we are currently stalled."]
                pub fn stalled(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::stalled::Stalled,
                    ::subxt::storage::address::Yes,
                    (),
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Grandpa",
                        "Stalled",
                        vec![],
                        [
                            6u8, 81u8, 205u8, 142u8, 195u8, 48u8, 0u8, 247u8, 108u8, 170u8, 10u8,
                            249u8, 72u8, 206u8, 32u8, 103u8, 109u8, 57u8, 51u8, 21u8, 144u8, 204u8,
                            79u8, 8u8, 191u8, 185u8, 38u8, 34u8, 118u8, 223u8, 75u8, 241u8,
                        ],
                    )
                }

                #[doc = " The number of changes (both in terms of keys and underlying economic responsibilities)"]
                #[doc = " in the \"set\" of Grandpa validators from genesis."]
                pub fn current_set_id(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::current_set_id::CurrentSetId,
                    ::subxt::storage::address::Yes,
                    ::subxt::storage::address::Yes,
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Grandpa",
                        "CurrentSetId",
                        vec![],
                        [
                            234u8, 215u8, 218u8, 42u8, 30u8, 76u8, 129u8, 40u8, 125u8, 137u8,
                            207u8, 47u8, 46u8, 213u8, 159u8, 50u8, 175u8, 81u8, 155u8, 123u8,
                            246u8, 175u8, 156u8, 68u8, 22u8, 113u8, 135u8, 137u8, 163u8, 18u8,
                            115u8, 73u8,
                        ],
                    )
                }

                #[doc = " A mapping from grandpa set ID to the index of the *most recent* session for which its"]
                #[doc = " members were responsible."]
                #[doc = ""]
                #[doc = " This is only used for validating equivocation proofs. An equivocation proof must"]
                #[doc = " contains a key-ownership proof for a given session, therefore we need a way to tie"]
                #[doc = " together sessions and GRANDPA set ids, i.e. we need to validate that a validator"]
                #[doc = " was the owner of a given key on a given session, and what the active set ID was"]
                #[doc = " during that session."]
                #[doc = ""]
                #[doc = " TWOX-NOTE: `SetId` is not under user control."]
                pub fn set_id_session_iter(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::set_id_session::SetIdSession,
                    (),
                    (),
                    ::subxt::storage::address::Yes,
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Grandpa",
                        "SetIdSession",
                        vec![],
                        [
                            47u8, 0u8, 239u8, 121u8, 187u8, 213u8, 254u8, 50u8, 238u8, 10u8, 162u8,
                            65u8, 189u8, 166u8, 37u8, 74u8, 82u8, 81u8, 160u8, 20u8, 180u8, 253u8,
                            238u8, 18u8, 209u8, 203u8, 38u8, 148u8, 16u8, 105u8, 72u8, 169u8,
                        ],
                    )
                }

                #[doc = " A mapping from grandpa set ID to the index of the *most recent* session for which its"]
                #[doc = " members were responsible."]
                #[doc = ""]
                #[doc = " This is only used for validating equivocation proofs. An equivocation proof must"]
                #[doc = " contains a key-ownership proof for a given session, therefore we need a way to tie"]
                #[doc = " together sessions and GRANDPA set ids, i.e. we need to validate that a validator"]
                #[doc = " was the owner of a given key on a given session, and what the active set ID was"]
                #[doc = " during that session."]
                #[doc = ""]
                #[doc = " TWOX-NOTE: `SetId` is not under user control."]
                pub fn set_id_session(
                    &self,
                    _0: impl ::std::borrow::Borrow<types::set_id_session::Param0>,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::set_id_session::SetIdSession,
                    ::subxt::storage::address::Yes,
                    (),
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Grandpa",
                        "SetIdSession",
                        vec![::subxt::storage::address::make_static_storage_map_key(_0.borrow())],
                        [
                            47u8, 0u8, 239u8, 121u8, 187u8, 213u8, 254u8, 50u8, 238u8, 10u8, 162u8,
                            65u8, 189u8, 166u8, 37u8, 74u8, 82u8, 81u8, 160u8, 20u8, 180u8, 253u8,
                            238u8, 18u8, 209u8, 203u8, 38u8, 148u8, 16u8, 105u8, 72u8, 169u8,
                        ],
                    )
                }
            }
        }

        pub mod constants {
            use super::runtime_types;

            pub struct ConstantsApi;

            impl ConstantsApi {
                #[doc = " Max Authorities in use"]
                pub fn max_authorities(
                    &self,
                ) -> ::subxt::constants::Address<::core::primitive::u32> {
                    ::subxt::constants::Address::new_static(
                        "Grandpa",
                        "MaxAuthorities",
                        [
                            98u8, 252u8, 116u8, 72u8, 26u8, 180u8, 225u8, 83u8, 200u8, 157u8,
                            125u8, 151u8, 53u8, 76u8, 168u8, 26u8, 10u8, 9u8, 98u8, 68u8, 9u8,
                            178u8, 197u8, 113u8, 31u8, 79u8, 200u8, 90u8, 203u8, 100u8, 41u8,
                            145u8,
                        ],
                    )
                }

                #[doc = " The maximum number of nominators for each validator."]
                pub fn max_nominators(
                    &self,
                ) -> ::subxt::constants::Address<::core::primitive::u32> {
                    ::subxt::constants::Address::new_static(
                        "Grandpa",
                        "MaxNominators",
                        [
                            98u8, 252u8, 116u8, 72u8, 26u8, 180u8, 225u8, 83u8, 200u8, 157u8,
                            125u8, 151u8, 53u8, 76u8, 168u8, 26u8, 10u8, 9u8, 98u8, 68u8, 9u8,
                            178u8, 197u8, 113u8, 31u8, 79u8, 200u8, 90u8, 203u8, 100u8, 41u8,
                            145u8,
                        ],
                    )
                }

                #[doc = " The maximum number of entries to keep in the set id to session index mapping."]
                #[doc = ""]
                #[doc = " Since the `SetIdSession` map is only used for validating equivocations this"]
                #[doc = " value should relate to the bonding duration of whatever staking system is"]
                #[doc = " being used (if any). If equivocation handling is not enabled then this value"]
                #[doc = " can be zero."]
                pub fn max_set_id_session_entries(
                    &self,
                ) -> ::subxt::constants::Address<::core::primitive::u64> {
                    ::subxt::constants::Address::new_static(
                        "Grandpa",
                        "MaxSetIdSessionEntries",
                        [
                            128u8, 214u8, 205u8, 242u8, 181u8, 142u8, 124u8, 231u8, 190u8, 146u8,
                            59u8, 226u8, 157u8, 101u8, 103u8, 117u8, 249u8, 65u8, 18u8, 191u8,
                            103u8, 119u8, 53u8, 85u8, 81u8, 96u8, 220u8, 42u8, 184u8, 239u8, 42u8,
                            246u8,
                        ],
                    )
                }
            }
        }
    }

    pub mod sudo {
        use super::{root_mod, runtime_types};

        #[doc = "Error for the Sudo pallet"]
        pub type Error = runtime_types::pallet_sudo::pallet::Error;
        #[doc = "Contains a variant per dispatchable extrinsic that this pallet has."]
        pub type Call = runtime_types::pallet_sudo::pallet::Call;

        pub mod calls {
            use super::{root_mod, runtime_types};

            type DispatchError = runtime_types::sp_runtime::DispatchError;

            pub mod types {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::sudo`]."]
                pub struct Sudo {
                    pub call: ::std::boxed::Box<sudo::Call>,
                }

                pub mod sudo {
                    use super::runtime_types;

                    pub type Call = runtime_types::runtime_chronicle::RuntimeCall;
                }

                impl ::subxt::blocks::StaticExtrinsic for Sudo {
                    const CALL: &'static str = "sudo";
                    const PALLET: &'static str = "Sudo";
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::sudo_unchecked_weight`]."]
                pub struct SudoUncheckedWeight {
                    pub call: ::std::boxed::Box<sudo_unchecked_weight::Call>,
                    pub weight: sudo_unchecked_weight::Weight,
                }

                pub mod sudo_unchecked_weight {
                    use super::runtime_types;

                    pub type Call = runtime_types::runtime_chronicle::RuntimeCall;
                    pub type Weight = runtime_types::sp_weights::weight_v2::Weight;
                }

                impl ::subxt::blocks::StaticExtrinsic for SudoUncheckedWeight {
                    const CALL: &'static str = "sudo_unchecked_weight";
                    const PALLET: &'static str = "Sudo";
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::set_key`]."]
                pub struct SetKey {
                    pub new: set_key::New,
                }

                pub mod set_key {
                    use super::runtime_types;

                    pub type New = ::subxt::utils::MultiAddress<::subxt::utils::AccountId32, ()>;
                }

                impl ::subxt::blocks::StaticExtrinsic for SetKey {
                    const CALL: &'static str = "set_key";
                    const PALLET: &'static str = "Sudo";
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::sudo_as`]."]
                pub struct SudoAs {
                    pub who: sudo_as::Who,
                    pub call: ::std::boxed::Box<sudo_as::Call>,
                }

                pub mod sudo_as {
                    use super::runtime_types;

                    pub type Who = ::subxt::utils::MultiAddress<::subxt::utils::AccountId32, ()>;
                    pub type Call = runtime_types::runtime_chronicle::RuntimeCall;
                }

                impl ::subxt::blocks::StaticExtrinsic for SudoAs {
                    const CALL: &'static str = "sudo_as";
                    const PALLET: &'static str = "Sudo";
                }
            }

            pub struct TransactionApi;

            impl TransactionApi {
                #[doc = "See [`Pallet::sudo`]."]
                pub fn sudo(&self, call: types::sudo::Call) -> ::subxt::tx::Payload<types::Sudo> {
                    ::subxt::tx::Payload::new_static(
                        "Sudo",
                        "sudo",
                        types::Sudo { call: ::std::boxed::Box::new(call) },
                        [
                            237u8, 167u8, 188u8, 42u8, 53u8, 81u8, 128u8, 15u8, 168u8, 39u8, 8u8,
                            134u8, 49u8, 136u8, 251u8, 162u8, 141u8, 66u8, 243u8, 169u8, 44u8,
                            68u8, 82u8, 232u8, 87u8, 62u8, 193u8, 74u8, 68u8, 123u8, 241u8, 151u8,
                        ],
                    )
                }

                #[doc = "See [`Pallet::sudo_unchecked_weight`]."]
                pub fn sudo_unchecked_weight(
                    &self,
                    call: types::sudo_unchecked_weight::Call,
                    weight: types::sudo_unchecked_weight::Weight,
                ) -> ::subxt::tx::Payload<types::SudoUncheckedWeight> {
                    ::subxt::tx::Payload::new_static(
                        "Sudo",
                        "sudo_unchecked_weight",
                        types::SudoUncheckedWeight { call: ::std::boxed::Box::new(call), weight },
                        [
                            191u8, 162u8, 221u8, 59u8, 231u8, 212u8, 11u8, 144u8, 158u8, 138u8,
                            158u8, 143u8, 73u8, 173u8, 231u8, 102u8, 250u8, 172u8, 214u8, 16u8,
                            184u8, 159u8, 182u8, 106u8, 161u8, 61u8, 28u8, 251u8, 117u8, 159u8,
                            255u8, 124u8,
                        ],
                    )
                }

                #[doc = "See [`Pallet::set_key`]."]
                pub fn set_key(
                    &self,
                    new: types::set_key::New,
                ) -> ::subxt::tx::Payload<types::SetKey> {
                    ::subxt::tx::Payload::new_static(
                        "Sudo",
                        "set_key",
                        types::SetKey { new },
                        [
                            9u8, 73u8, 39u8, 205u8, 188u8, 127u8, 143u8, 54u8, 128u8, 94u8, 8u8,
                            227u8, 197u8, 44u8, 70u8, 93u8, 228u8, 196u8, 64u8, 165u8, 226u8,
                            158u8, 101u8, 192u8, 22u8, 193u8, 102u8, 84u8, 21u8, 35u8, 92u8, 198u8,
                        ],
                    )
                }

                #[doc = "See [`Pallet::sudo_as`]."]
                pub fn sudo_as(
                    &self,
                    who: types::sudo_as::Who,
                    call: types::sudo_as::Call,
                ) -> ::subxt::tx::Payload<types::SudoAs> {
                    ::subxt::tx::Payload::new_static(
                        "Sudo",
                        "sudo_as",
                        types::SudoAs { who, call: ::std::boxed::Box::new(call) },
                        [
                            230u8, 21u8, 191u8, 185u8, 54u8, 39u8, 134u8, 240u8, 51u8, 145u8,
                            105u8, 151u8, 191u8, 224u8, 205u8, 96u8, 71u8, 3u8, 149u8, 212u8, 92u8,
                            9u8, 75u8, 107u8, 144u8, 158u8, 151u8, 129u8, 2u8, 45u8, 228u8, 118u8,
                        ],
                    )
                }
            }
        }

        #[doc = "The `Event` enum of this pallet"]
        pub type Event = runtime_types::pallet_sudo::pallet::Event;

        pub mod events {
            use super::runtime_types;

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            #[doc = "A sudo call just took place."]
            pub struct Sudid {
                pub sudo_result: sudid::SudoResult,
            }

            pub mod sudid {
                use super::runtime_types;

                pub type SudoResult =
                ::core::result::Result<(), runtime_types::sp_runtime::DispatchError>;
            }

            impl ::subxt::events::StaticEvent for Sudid {
                const EVENT: &'static str = "Sudid";
                const PALLET: &'static str = "Sudo";
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            #[doc = "The sudo key has been updated."]
            pub struct KeyChanged {
                pub old_sudoer: key_changed::OldSudoer,
            }

            pub mod key_changed {
                use super::runtime_types;

                pub type OldSudoer = ::core::option::Option<::subxt::utils::AccountId32>;
            }

            impl ::subxt::events::StaticEvent for KeyChanged {
                const EVENT: &'static str = "KeyChanged";
                const PALLET: &'static str = "Sudo";
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            #[doc = "A [sudo_as](Pallet::sudo_as) call just took place."]
            pub struct SudoAsDone {
                pub sudo_result: sudo_as_done::SudoResult,
            }

            pub mod sudo_as_done {
                use super::runtime_types;

                pub type SudoResult =
                ::core::result::Result<(), runtime_types::sp_runtime::DispatchError>;
            }

            impl ::subxt::events::StaticEvent for SudoAsDone {
                const EVENT: &'static str = "SudoAsDone";
                const PALLET: &'static str = "Sudo";
            }
        }

        pub mod storage {
            use super::runtime_types;

            pub mod types {
                use super::runtime_types;

                pub mod key {
                    use super::runtime_types;

                    pub type Key = ::subxt::utils::AccountId32;
                }
            }

            pub struct StorageApi;

            impl StorageApi {
                #[doc = " The `AccountId` of the sudo key."]
                pub fn key(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::key::Key,
                    ::subxt::storage::address::Yes,
                    (),
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Sudo",
                        "Key",
                        vec![],
                        [
                            72u8, 14u8, 225u8, 162u8, 205u8, 247u8, 227u8, 105u8, 116u8, 57u8, 4u8,
                            31u8, 84u8, 137u8, 227u8, 228u8, 133u8, 245u8, 206u8, 227u8, 117u8,
                            36u8, 252u8, 151u8, 107u8, 15u8, 180u8, 4u8, 4u8, 152u8, 195u8, 144u8,
                        ],
                    )
                }
            }
        }
    }

    pub mod chronicle {
        use super::{root_mod, runtime_types};

        #[doc = "The `Error` enum of this pallet."]
        pub type Error = runtime_types::pallet_chronicle::pallet::Error;
        #[doc = "Contains a variant per dispatchable extrinsic that this pallet has."]
        pub type Call = runtime_types::pallet_chronicle::pallet::Call;

        pub mod calls {
            use super::{root_mod, runtime_types};

            type DispatchError = runtime_types::sp_runtime::DispatchError;

            pub mod types {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::apply`]."]
                pub struct Apply {
                    pub operations: apply::Operations,
                }

                pub mod apply {
                    use super::runtime_types;

                    pub type Operations = runtime_types::common::ledger::OperationSubmission;
                }

                impl ::subxt::blocks::StaticExtrinsic for Apply {
                    const CALL: &'static str = "apply";
                    const PALLET: &'static str = "Chronicle";
                }
            }

            pub struct TransactionApi;

            impl TransactionApi {
                #[doc = "See [`Pallet::apply`]."]
                pub fn apply(
                    &self,
                    operations: types::apply::Operations,
                ) -> ::subxt::tx::Payload<types::Apply> {
                    ::subxt::tx::Payload::new_static(
                        "Chronicle",
                        "apply",
                        types::Apply { operations },
                        [
                            155u8, 52u8, 231u8, 240u8, 97u8, 198u8, 219u8, 89u8, 101u8, 93u8, 60u8,
                            206u8, 9u8, 244u8, 58u8, 69u8, 176u8, 61u8, 109u8, 126u8, 75u8, 178u8,
                            195u8, 2u8, 158u8, 67u8, 158u8, 143u8, 9u8, 252u8, 61u8, 139u8,
                        ],
                    )
                }
            }
        }

        #[doc = "The `Event` enum of this pallet"]
        pub type Event = runtime_types::pallet_chronicle::pallet::Event;

        pub mod events {
            use super::runtime_types;

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct Applied(pub applied::Field0, pub applied::Field1, pub applied::Field2);

            pub mod applied {
                use super::runtime_types;

                pub type Field0 = runtime_types::common::prov::model::ProvModel;
                pub type Field1 = runtime_types::common::identity::SignedIdentity;
                pub type Field2 = [::core::primitive::u8; 16usize];
            }

            impl ::subxt::events::StaticEvent for Applied {
                const EVENT: &'static str = "Applied";
                const PALLET: &'static str = "Chronicle";
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct Contradiction(
                pub contradiction::Field0,
                pub contradiction::Field1,
                pub contradiction::Field2,
            );

            pub mod contradiction {
                use super::runtime_types;

                pub type Field0 = runtime_types::common::prov::model::contradiction::Contradiction;
                pub type Field1 = runtime_types::common::identity::SignedIdentity;
                pub type Field2 = [::core::primitive::u8; 16usize];
            }

            impl ::subxt::events::StaticEvent for Contradiction {
                const EVENT: &'static str = "Contradiction";
                const PALLET: &'static str = "Chronicle";
            }
        }

        pub mod storage {
            use super::runtime_types;

            pub mod types {
                use super::runtime_types;

                pub mod provenance {
                    use super::runtime_types;

                    pub type Provenance = runtime_types::common::prov::model::ProvModel;
                    pub type Param0 = runtime_types::common::ledger::ChronicleAddress;
                }

                pub mod opa_settings {
                    use super::runtime_types;

                    pub type OpaSettings =
                    ::core::option::Option<runtime_types::common::opa::core::OpaSettings>;
                }
            }

            pub struct StorageApi;

            impl StorageApi {
                pub fn provenance_iter(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::provenance::Provenance,
                    (),
                    (),
                    ::subxt::storage::address::Yes,
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Chronicle",
                        "Provenance",
                        vec![],
                        [
                            89u8, 248u8, 19u8, 1u8, 96u8, 148u8, 157u8, 57u8, 205u8, 85u8, 157u8,
                            10u8, 226u8, 22u8, 253u8, 189u8, 239u8, 95u8, 193u8, 72u8, 200u8, 4u8,
                            236u8, 165u8, 80u8, 97u8, 150u8, 137u8, 142u8, 46u8, 37u8, 37u8,
                        ],
                    )
                }

                pub fn provenance(
                    &self,
                    _0: impl ::std::borrow::Borrow<types::provenance::Param0>,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::provenance::Provenance,
                    ::subxt::storage::address::Yes,
                    (),
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Chronicle",
                        "Provenance",
                        vec![::subxt::storage::address::make_static_storage_map_key(_0.borrow())],
                        [
                            89u8, 248u8, 19u8, 1u8, 96u8, 148u8, 157u8, 57u8, 205u8, 85u8, 157u8,
                            10u8, 226u8, 22u8, 253u8, 189u8, 239u8, 95u8, 193u8, 72u8, 200u8, 4u8,
                            236u8, 165u8, 80u8, 97u8, 150u8, 137u8, 142u8, 46u8, 37u8, 37u8,
                        ],
                    )
                }

                pub fn opa_settings(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::opa_settings::OpaSettings,
                    ::subxt::storage::address::Yes,
                    (),
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Chronicle",
                        "OpaSettings",
                        vec![],
                        [
                            147u8, 57u8, 197u8, 174u8, 245u8, 128u8, 185u8, 25u8, 59u8, 211u8,
                            103u8, 144u8, 133u8, 65u8, 135u8, 26u8, 249u8, 122u8, 98u8, 119u8,
                            113u8, 203u8, 8u8, 216u8, 144u8, 203u8, 28u8, 152u8, 76u8, 108u8, 17u8,
                            149u8,
                        ],
                    )
                }
            }
        }
    }

    pub mod opa {
        use super::{root_mod, runtime_types};

        #[doc = "The `Error` enum of this pallet."]
        pub type Error = runtime_types::pallet_opa::pallet::Error;
        #[doc = "Contains a variant per dispatchable extrinsic that this pallet has."]
        pub type Call = runtime_types::pallet_opa::pallet::Call;

        pub mod calls {
            use super::{root_mod, runtime_types};

            type DispatchError = runtime_types::sp_runtime::DispatchError;

            pub mod types {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "See [`Pallet::apply`]."]
                pub struct Apply {
                    pub submission: apply::Submission,
                }

                pub mod apply {
                    use super::runtime_types;

                    pub type Submission = runtime_types::common::opa::core::codec::OpaSubmissionV1;
                }

                impl ::subxt::blocks::StaticExtrinsic for Apply {
                    const CALL: &'static str = "apply";
                    const PALLET: &'static str = "Opa";
                }
            }

            pub struct TransactionApi;

            impl TransactionApi {
                #[doc = "See [`Pallet::apply`]."]
                pub fn apply(
                    &self,
                    submission: types::apply::Submission,
                ) -> ::subxt::tx::Payload<types::Apply> {
                    ::subxt::tx::Payload::new_static(
                        "Opa",
                        "apply",
                        types::Apply { submission },
                        [
                            234u8, 194u8, 237u8, 194u8, 37u8, 84u8, 219u8, 67u8, 228u8, 177u8,
                            34u8, 4u8, 100u8, 5u8, 196u8, 246u8, 3u8, 28u8, 65u8, 188u8, 66u8,
                            227u8, 204u8, 143u8, 143u8, 230u8, 11u8, 229u8, 77u8, 61u8, 108u8,
                            19u8,
                        ],
                    )
                }
            }
        }

        #[doc = "The `Event` enum of this pallet"]
        pub type Event = runtime_types::pallet_opa::pallet::Event;

        pub mod events {
            use super::runtime_types;

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct PolicyUpdate(pub policy_update::Field0, pub policy_update::Field1);

            pub mod policy_update {
                use super::runtime_types;

                pub type Field0 = runtime_types::common::opa::core::codec::PolicyMetaV1;
                pub type Field1 = runtime_types::common::prov::model::ChronicleTransactionId;
            }

            impl ::subxt::events::StaticEvent for PolicyUpdate {
                const EVENT: &'static str = "PolicyUpdate";
                const PALLET: &'static str = "Opa";
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct KeyUpdate(pub key_update::Field0, pub key_update::Field1);

            pub mod key_update {
                use super::runtime_types;

                pub type Field0 = runtime_types::common::opa::core::codec::KeysV1;
                pub type Field1 = runtime_types::common::prov::model::ChronicleTransactionId;
            }

            impl ::subxt::events::StaticEvent for KeyUpdate {
                const EVENT: &'static str = "KeyUpdate";
                const PALLET: &'static str = "Opa";
            }
        }

        pub mod storage {
            use super::runtime_types;

            pub mod types {
                use super::runtime_types;

                pub mod policy_store {
                    use super::runtime_types;

                    pub type PolicyStore = runtime_types::common::opa::core::codec::PolicyV1;
                    pub type Param0 = runtime_types::common::opa::core::PolicyAddress;
                }

                pub mod policy_meta_store {
                    use super::runtime_types;

                    pub type PolicyMetaStore =
                    runtime_types::common::opa::core::codec::PolicyMetaV1;
                    pub type Param0 = runtime_types::common::opa::core::PolicyMetaAddress;
                }

                pub mod key_store {
                    use super::runtime_types;

                    pub type KeyStore = runtime_types::common::opa::core::codec::KeysV1;
                    pub type Param0 = runtime_types::common::opa::core::KeyAddress;
                }
            }

            pub struct StorageApi;

            impl StorageApi {
                pub fn policy_store_iter(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::policy_store::PolicyStore,
                    (),
                    (),
                    ::subxt::storage::address::Yes,
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Opa",
                        "PolicyStore",
                        vec![],
                        [
                            196u8, 59u8, 202u8, 150u8, 122u8, 101u8, 78u8, 223u8, 11u8, 18u8,
                            183u8, 27u8, 146u8, 220u8, 7u8, 187u8, 13u8, 188u8, 251u8, 24u8, 81u8,
                            227u8, 248u8, 52u8, 162u8, 245u8, 38u8, 136u8, 20u8, 90u8, 197u8,
                            181u8,
                        ],
                    )
                }

                pub fn policy_store(
                    &self,
                    _0: impl ::std::borrow::Borrow<types::policy_store::Param0>,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::policy_store::PolicyStore,
                    ::subxt::storage::address::Yes,
                    (),
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Opa",
                        "PolicyStore",
                        vec![::subxt::storage::address::make_static_storage_map_key(_0.borrow())],
                        [
                            196u8, 59u8, 202u8, 150u8, 122u8, 101u8, 78u8, 223u8, 11u8, 18u8,
                            183u8, 27u8, 146u8, 220u8, 7u8, 187u8, 13u8, 188u8, 251u8, 24u8, 81u8,
                            227u8, 248u8, 52u8, 162u8, 245u8, 38u8, 136u8, 20u8, 90u8, 197u8,
                            181u8,
                        ],
                    )
                }

                pub fn policy_meta_store_iter(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::policy_meta_store::PolicyMetaStore,
                    (),
                    (),
                    ::subxt::storage::address::Yes,
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Opa",
                        "PolicyMetaStore",
                        vec![],
                        [
                            189u8, 108u8, 84u8, 44u8, 122u8, 229u8, 34u8, 242u8, 101u8, 92u8,
                            113u8, 242u8, 103u8, 46u8, 77u8, 145u8, 102u8, 172u8, 41u8, 9u8, 189u8,
                            102u8, 178u8, 31u8, 0u8, 164u8, 222u8, 220u8, 240u8, 91u8, 126u8,
                            249u8,
                        ],
                    )
                }

                pub fn policy_meta_store(
                    &self,
                    _0: impl ::std::borrow::Borrow<types::policy_meta_store::Param0>,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::policy_meta_store::PolicyMetaStore,
                    ::subxt::storage::address::Yes,
                    (),
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Opa",
                        "PolicyMetaStore",
                        vec![::subxt::storage::address::make_static_storage_map_key(_0.borrow())],
                        [
                            189u8, 108u8, 84u8, 44u8, 122u8, 229u8, 34u8, 242u8, 101u8, 92u8,
                            113u8, 242u8, 103u8, 46u8, 77u8, 145u8, 102u8, 172u8, 41u8, 9u8, 189u8,
                            102u8, 178u8, 31u8, 0u8, 164u8, 222u8, 220u8, 240u8, 91u8, 126u8,
                            249u8,
                        ],
                    )
                }

                pub fn key_store_iter(
                    &self,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::key_store::KeyStore,
                    (),
                    (),
                    ::subxt::storage::address::Yes,
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Opa",
                        "KeyStore",
                        vec![],
                        [
                            169u8, 58u8, 57u8, 223u8, 245u8, 168u8, 156u8, 245u8, 216u8, 144u8,
                            41u8, 29u8, 118u8, 241u8, 23u8, 231u8, 77u8, 178u8, 178u8, 237u8, 99u8,
                            240u8, 13u8, 53u8, 27u8, 166u8, 232u8, 13u8, 61u8, 138u8, 218u8, 67u8,
                        ],
                    )
                }

                pub fn key_store(
                    &self,
                    _0: impl ::std::borrow::Borrow<types::key_store::Param0>,
                ) -> ::subxt::storage::address::Address<
                    ::subxt::storage::address::StaticStorageMapKey,
                    types::key_store::KeyStore,
                    ::subxt::storage::address::Yes,
                    (),
                    (),
                > {
                    ::subxt::storage::address::Address::new_static(
                        "Opa",
                        "KeyStore",
                        vec![::subxt::storage::address::make_static_storage_map_key(_0.borrow())],
                        [
                            169u8, 58u8, 57u8, 223u8, 245u8, 168u8, 156u8, 245u8, 216u8, 144u8,
                            41u8, 29u8, 118u8, 241u8, 23u8, 231u8, 77u8, 178u8, 178u8, 237u8, 99u8,
                            240u8, 13u8, 53u8, 27u8, 166u8, 232u8, 13u8, 61u8, 138u8, 218u8, 67u8,
                        ],
                    )
                }
            }
        }
    }

    pub mod runtime_types {
        use super::runtime_types;

        pub mod bounded_collections {
            use super::runtime_types;

            pub mod bounded_vec {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct BoundedVec<_0>(pub ::std::vec::Vec<_0>);
            }

            pub mod weak_bounded_vec {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct WeakBoundedVec<_0>(pub ::std::vec::Vec<_0>);
            }
        }

        pub mod common {
            use super::runtime_types;

            pub mod attributes {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct Attribute {
                    pub typ: ::std::string::String,
                    pub value: runtime_types::common::attributes::SerdeWrapper,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct Attributes {
                    pub typ: ::core::option::Option<runtime_types::common::prov::id::DomaintypeId>,
                    pub items: ::subxt::utils::KeyedVec<
                        ::std::string::String,
                        runtime_types::common::attributes::Attribute,
                    >,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct SerdeWrapper(pub ::std::string::String);
            }

            pub mod identity {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct SignedIdentity {
                    pub identity: ::std::string::String,
                    pub signature: ::core::option::Option<::std::vec::Vec<::core::primitive::u8>>,
                    pub verifying_key:
                    ::core::option::Option<::std::vec::Vec<::core::primitive::u8>>,
                }
            }

            pub mod ledger {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct ChronicleAddress {
                    pub namespace:
                    ::core::option::Option<runtime_types::common::prov::id::NamespaceId>,
                    pub resource: runtime_types::common::prov::id::ChronicleIri,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct OperationSubmission {
                    pub correlation_id: [::core::primitive::u8; 16usize],
                    pub items: ::std::vec::Vec<
                        runtime_types::common::prov::operations::ChronicleOperation,
                    >,
                    pub identity: runtime_types::common::identity::SignedIdentity,
                }
            }

            pub mod opa {
                use super::runtime_types;

                pub mod core {
                    use super::runtime_types;

                    pub mod codec {
                        use super::runtime_types;

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub struct BootstrapRootV1 {
                            pub public_key: runtime_types::common::opa::core::PemEncoded,
                        }

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub struct KeyRegistrationV1 {
                            pub key: runtime_types::common::opa::core::PemEncoded,
                            pub version: ::core::primitive::u64,
                        }

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub struct KeysV1 {
                            pub id: ::std::string::String,
                            pub current: runtime_types::common::opa::core::codec::KeyRegistrationV1,
                            pub expired: ::core::option::Option<
                                runtime_types::common::opa::core::codec::KeyRegistrationV1,
                            >,
                        }

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub struct NewPublicKeyV1 {
                            pub public_key: runtime_types::common::opa::core::PemEncoded,
                            pub id: ::std::string::String,
                        }

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub struct OpaSubmissionV1 {
                            pub version: ::std::string::String,
                            pub correlation_id: [::core::primitive::u8; 16usize],
                            pub span_id: ::core::primitive::u64,
                            pub payload: runtime_types::common::opa::core::codec::PayloadV1,
                        }

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub enum OperationV1 {
                            #[codec(index = 0)]
                            RegisterKey(runtime_types::common::opa::core::codec::RegisterKeyV1),
                            #[codec(index = 1)]
                            RotateKey(runtime_types::common::opa::core::codec::RotateKeyV1),
                            #[codec(index = 2)]
                            SetPolicy(runtime_types::common::opa::core::codec::SetPolicyV1),
                        }

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub enum PayloadV1 {
                            #[codec(index = 0)]
                            BootstrapRoot(runtime_types::common::opa::core::codec::BootstrapRootV1),
                            #[codec(index = 1)]
                            SignedOperation(
                                runtime_types::common::opa::core::codec::SignedOperationV1,
                            ),
                        }

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub struct PolicyMetaV1 {
                            pub id: ::std::string::String,
                            pub hash: runtime_types::common::opa::core::H128,
                            pub policy_address: runtime_types::common::opa::core::PolicyAddress,
                        }

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub struct PolicyV1(pub ::std::vec::Vec<::core::primitive::u8>);

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub struct RegisterKeyV1 {
                            pub public_key: runtime_types::common::opa::core::PemEncoded,
                            pub id: ::std::string::String,
                            pub overwrite_existing: ::core::primitive::bool,
                        }

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub struct RotateKeyV1 {
                            pub payload: runtime_types::common::opa::core::codec::NewPublicKeyV1,
                            pub previous_signing_key: runtime_types::common::opa::core::PemEncoded,
                            pub previous_signature: ::std::vec::Vec<::core::primitive::u8>,
                            pub new_signing_key: runtime_types::common::opa::core::PemEncoded,
                            pub new_signature: ::std::vec::Vec<::core::primitive::u8>,
                        }

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub struct SetPolicyV1 {
                            pub id: ::std::string::String,
                            pub policy: runtime_types::common::opa::core::Policy,
                        }

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub struct SignedOperationPayloadV1 {
                            pub operation: runtime_types::common::opa::core::codec::OperationV1,
                        }

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub struct SignedOperationV1 {
                            pub payload:
                            runtime_types::common::opa::core::codec::SignedOperationPayloadV1,
                            pub verifying_key: runtime_types::common::opa::core::PemEncoded,
                            pub signature: ::std::vec::Vec<::core::primitive::u8>,
                        }
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct H128(pub [::core::primitive::u8; 16usize]);

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct KeyAddress(pub runtime_types::common::opa::core::H128);

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct OpaSettings {
                        pub policy_address: runtime_types::common::opa::core::PolicyAddress,
                        pub policy_name: ::std::string::String,
                        pub entrypoint: ::std::string::String,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct PemEncoded(pub ::std::string::String);

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Policy(pub ::std::vec::Vec<::core::primitive::u8>);

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct PolicyAddress(pub runtime_types::common::opa::core::H128);

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct PolicyMetaAddress(pub runtime_types::common::opa::core::H128);
                }
            }

            pub mod prov {
                use super::runtime_types;

                pub mod id {
                    use super::runtime_types;

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct ActivityId(pub runtime_types::common::prov::id::ExternalId);

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct AgentId(pub runtime_types::common::prov::id::ExternalId);

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct AssociationId {
                        pub agent: runtime_types::common::prov::id::ExternalId,
                        pub activity: runtime_types::common::prov::id::ExternalId,
                        pub role: ::core::option::Option<runtime_types::common::prov::id::Role>,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct AttributionId {
                        pub agent: runtime_types::common::prov::id::ExternalId,
                        pub entity: runtime_types::common::prov::id::ExternalId,
                        pub role: ::core::option::Option<runtime_types::common::prov::id::Role>,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub enum ChronicleIri {
                        #[codec(index = 0)]
                        Namespace(runtime_types::common::prov::id::NamespaceId),
                        #[codec(index = 1)]
                        Domaintype(runtime_types::common::prov::id::DomaintypeId),
                        #[codec(index = 2)]
                        Entity(runtime_types::common::prov::id::EntityId),
                        #[codec(index = 3)]
                        Agent(runtime_types::common::prov::id::AgentId),
                        #[codec(index = 4)]
                        Activity(runtime_types::common::prov::id::ActivityId),
                        #[codec(index = 5)]
                        Association(runtime_types::common::prov::id::AssociationId),
                        #[codec(index = 6)]
                        Attribution(runtime_types::common::prov::id::AttributionId),
                        #[codec(index = 7)]
                        Delegation(runtime_types::common::prov::id::DelegationId),
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct DelegationId {
                        pub delegate: runtime_types::common::prov::id::ExternalId,
                        pub responsible: runtime_types::common::prov::id::ExternalId,
                        pub activity:
                        ::core::option::Option<runtime_types::common::prov::id::ExternalId>,
                        pub role: ::core::option::Option<runtime_types::common::prov::id::Role>,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct DomaintypeId(pub runtime_types::common::prov::id::ExternalId);

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct EntityId(pub runtime_types::common::prov::id::ExternalId);

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct ExternalId(pub ::std::string::String);

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct NamespaceId {
                        pub external_id: runtime_types::common::prov::id::ExternalId,
                        pub uuid: [::core::primitive::u8; 16usize],
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Role(pub ::std::string::String);
                }

                pub mod model {
                    use super::runtime_types;

                    pub mod contradiction {
                        use super::runtime_types;

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub struct Contradiction {
                            pub id: runtime_types::common::prov::id::ChronicleIri,
                            pub namespace: runtime_types::common::prov::id::NamespaceId,
                            pub contradiction: ::std::vec::Vec<runtime_types::common::prov::model::contradiction::ContradictionDetail>,
                        }

                        #[derive(
                            ::subxt::ext::codec::Decode,
                            ::subxt::ext::codec::Encode,
                            ::subxt::ext::scale_decode::DecodeAsType,
                            ::subxt::ext::scale_encode::EncodeAsType,
                            Debug,
                        )]
                        #[codec(crate =::subxt::ext::codec)]
                        #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                        #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                        pub enum ContradictionDetail {
                            #[codec(index = 0)]
                            AttributeValueChange {
                                name: ::std::string::String,
                                value: runtime_types::common::attributes::Attribute,
                                attempted: runtime_types::common::attributes::Attribute,
                            },
                            #[codec(index = 1)]
                            StartAlteration {
                                value: runtime_types::common::prov::operations::TimeWrapper,
                                attempted: runtime_types::common::prov::operations::TimeWrapper,
                            },
                            #[codec(index = 2)]
                            EndAlteration {
                                value: runtime_types::common::prov::operations::TimeWrapper,
                                attempted: runtime_types::common::prov::operations::TimeWrapper,
                            },
                            #[codec(index = 3)]
                            InvalidRange {
                                start: runtime_types::common::prov::operations::TimeWrapper,
                                end: runtime_types::common::prov::operations::TimeWrapper,
                            },
                        }
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Activity {
                        pub id: runtime_types::common::prov::id::ActivityId,
                        pub namespace_id: runtime_types::common::prov::id::NamespaceId,
                        pub external_id: runtime_types::common::prov::id::ExternalId,
                        pub domaintype_id:
                        ::core::option::Option<runtime_types::common::prov::id::DomaintypeId>,
                        pub attributes: ::subxt::utils::KeyedVec<
                            ::std::string::String,
                            runtime_types::common::attributes::Attribute,
                        >,
                        pub started: ::core::option::Option<
                            runtime_types::common::prov::operations::TimeWrapper,
                        >,
                        pub ended: ::core::option::Option<
                            runtime_types::common::prov::operations::TimeWrapper,
                        >,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Agent {
                        pub id: runtime_types::common::prov::id::AgentId,
                        pub namespaceid: runtime_types::common::prov::id::NamespaceId,
                        pub external_id: runtime_types::common::prov::id::ExternalId,
                        pub domaintypeid:
                        ::core::option::Option<runtime_types::common::prov::id::DomaintypeId>,
                        pub attributes: ::subxt::utils::KeyedVec<
                            ::std::string::String,
                            runtime_types::common::attributes::Attribute,
                        >,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Association {
                        pub namespace_id: runtime_types::common::prov::id::NamespaceId,
                        pub id: runtime_types::common::prov::id::AssociationId,
                        pub agent_id: runtime_types::common::prov::id::AgentId,
                        pub activity_id: runtime_types::common::prov::id::ActivityId,
                        pub role: ::core::option::Option<runtime_types::common::prov::id::Role>,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Attribution {
                        pub namespace_id: runtime_types::common::prov::id::NamespaceId,
                        pub id: runtime_types::common::prov::id::AttributionId,
                        pub agent_id: runtime_types::common::prov::id::AgentId,
                        pub entity_id: runtime_types::common::prov::id::EntityId,
                        pub role: ::core::option::Option<runtime_types::common::prov::id::Role>,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct ChronicleTransactionId(pub [::core::primitive::u8; 16usize]);

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Delegation {
                        pub namespace_id: runtime_types::common::prov::id::NamespaceId,
                        pub id: runtime_types::common::prov::id::DelegationId,
                        pub delegate_id: runtime_types::common::prov::id::AgentId,
                        pub responsible_id: runtime_types::common::prov::id::AgentId,
                        pub activity_id:
                        ::core::option::Option<runtime_types::common::prov::id::ActivityId>,
                        pub role: ::core::option::Option<runtime_types::common::prov::id::Role>,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Derivation {
                        pub generated_id: runtime_types::common::prov::id::EntityId,
                        pub used_id: runtime_types::common::prov::id::EntityId,
                        pub activity_id:
                        ::core::option::Option<runtime_types::common::prov::id::ActivityId>,
                        pub typ: runtime_types::common::prov::operations::DerivationType,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Entity {
                        pub id: runtime_types::common::prov::id::EntityId,
                        pub namespace_id: runtime_types::common::prov::id::NamespaceId,
                        pub external_id: runtime_types::common::prov::id::ExternalId,
                        pub domaintypeid:
                        ::core::option::Option<runtime_types::common::prov::id::DomaintypeId>,
                        pub attributes: ::subxt::utils::KeyedVec<
                            ::std::string::String,
                            runtime_types::common::attributes::Attribute,
                        >,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct GeneratedEntity {
                        pub entity_id: runtime_types::common::prov::id::EntityId,
                        pub generated_id: runtime_types::common::prov::id::ActivityId,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Generation {
                        pub activity_id: runtime_types::common::prov::id::ActivityId,
                        pub generated_id: runtime_types::common::prov::id::EntityId,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Namespace {
                        pub id: runtime_types::common::prov::id::NamespaceId,
                        pub uuid: [::core::primitive::u8; 16usize],
                        pub external_id: runtime_types::common::prov::id::ExternalId,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct ProvModel {
                        pub namespaces: ::subxt::utils::KeyedVec<
                            runtime_types::common::prov::id::NamespaceId,
                            runtime_types::common::prov::model::Namespace,
                        >,
                        pub agents: ::subxt::utils::KeyedVec<
                            (
                                runtime_types::common::prov::id::NamespaceId,
                                runtime_types::common::prov::id::AgentId,
                            ),
                            runtime_types::common::prov::model::Agent,
                        >,
                        pub acted_on_behalf_of: ::subxt::utils::KeyedVec<
                            (
                                runtime_types::common::prov::id::NamespaceId,
                                runtime_types::common::prov::id::AgentId,
                            ),
                            ::std::vec::Vec<runtime_types::common::prov::model::Delegation>,
                        >,
                        pub delegation: ::subxt::utils::KeyedVec<
                            (
                                runtime_types::common::prov::id::NamespaceId,
                                runtime_types::common::prov::id::AgentId,
                            ),
                            ::std::vec::Vec<runtime_types::common::prov::model::Delegation>,
                        >,
                        pub entities: ::subxt::utils::KeyedVec<
                            (
                                runtime_types::common::prov::id::NamespaceId,
                                runtime_types::common::prov::id::EntityId,
                            ),
                            runtime_types::common::prov::model::Entity,
                        >,
                        pub derivation: ::subxt::utils::KeyedVec<
                            (
                                runtime_types::common::prov::id::NamespaceId,
                                runtime_types::common::prov::id::EntityId,
                            ),
                            ::std::vec::Vec<runtime_types::common::prov::model::Derivation>,
                        >,
                        pub generation: ::subxt::utils::KeyedVec<
                            (
                                runtime_types::common::prov::id::NamespaceId,
                                runtime_types::common::prov::id::EntityId,
                            ),
                            ::std::vec::Vec<runtime_types::common::prov::model::Generation>,
                        >,
                        pub attribution: ::subxt::utils::KeyedVec<
                            (
                                runtime_types::common::prov::id::NamespaceId,
                                runtime_types::common::prov::id::EntityId,
                            ),
                            ::std::vec::Vec<runtime_types::common::prov::model::Attribution>,
                        >,
                        pub activities: ::subxt::utils::KeyedVec<
                            (
                                runtime_types::common::prov::id::NamespaceId,
                                runtime_types::common::prov::id::ActivityId,
                            ),
                            runtime_types::common::prov::model::Activity,
                        >,
                        pub was_informed_by: ::subxt::utils::KeyedVec<
                            (
                                runtime_types::common::prov::id::NamespaceId,
                                runtime_types::common::prov::id::ActivityId,
                            ),
                            ::std::vec::Vec<(
                                runtime_types::common::prov::id::NamespaceId,
                                runtime_types::common::prov::id::ActivityId,
                            )>,
                        >,
                        pub generated: ::subxt::utils::KeyedVec<
                            (
                                runtime_types::common::prov::id::NamespaceId,
                                runtime_types::common::prov::id::ActivityId,
                            ),
                            ::std::vec::Vec<runtime_types::common::prov::model::GeneratedEntity>,
                        >,
                        pub association: ::subxt::utils::KeyedVec<
                            (
                                runtime_types::common::prov::id::NamespaceId,
                                runtime_types::common::prov::id::ActivityId,
                            ),
                            ::std::vec::Vec<runtime_types::common::prov::model::Association>,
                        >,
                        pub usage: ::subxt::utils::KeyedVec<
                            (
                                runtime_types::common::prov::id::NamespaceId,
                                runtime_types::common::prov::id::ActivityId,
                            ),
                            ::std::vec::Vec<runtime_types::common::prov::model::Usage>,
                        >,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Usage {
                        pub activity_id: runtime_types::common::prov::id::ActivityId,
                        pub entity_id: runtime_types::common::prov::id::EntityId,
                    }
                }

                pub mod operations {
                    use super::runtime_types;

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct ActivityExists {
                        pub namespace: runtime_types::common::prov::id::NamespaceId,
                        pub external_id: runtime_types::common::prov::id::ExternalId,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct ActivityUses {
                        pub namespace: runtime_types::common::prov::id::NamespaceId,
                        pub id: runtime_types::common::prov::id::EntityId,
                        pub activity: runtime_types::common::prov::id::ActivityId,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct ActsOnBehalfOf {
                        pub id: runtime_types::common::prov::id::DelegationId,
                        pub role: ::core::option::Option<runtime_types::common::prov::id::Role>,
                        pub activity_id:
                        ::core::option::Option<runtime_types::common::prov::id::ActivityId>,
                        pub responsible_id: runtime_types::common::prov::id::AgentId,
                        pub delegate_id: runtime_types::common::prov::id::AgentId,
                        pub namespace: runtime_types::common::prov::id::NamespaceId,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct AgentExists {
                        pub namespace: runtime_types::common::prov::id::NamespaceId,
                        pub external_id: runtime_types::common::prov::id::ExternalId,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub enum ChronicleOperation {
                        #[codec(index = 0)]
                        CreateNamespace(runtime_types::common::prov::operations::CreateNamespace),
                        #[codec(index = 1)]
                        AgentExists(runtime_types::common::prov::operations::AgentExists),
                        #[codec(index = 2)]
                        AgentActsOnBehalfOf(
                            runtime_types::common::prov::operations::ActsOnBehalfOf,
                        ),
                        #[codec(index = 3)]
                        ActivityExists(runtime_types::common::prov::operations::ActivityExists),
                        #[codec(index = 4)]
                        StartActivity(runtime_types::common::prov::operations::StartActivity),
                        #[codec(index = 5)]
                        EndActivity(runtime_types::common::prov::operations::EndActivity),
                        #[codec(index = 6)]
                        ActivityUses(runtime_types::common::prov::operations::ActivityUses),
                        #[codec(index = 7)]
                        EntityExists(runtime_types::common::prov::operations::EntityExists),
                        #[codec(index = 8)]
                        WasGeneratedBy(runtime_types::common::prov::operations::WasGeneratedBy),
                        #[codec(index = 9)]
                        EntityDerive(runtime_types::common::prov::operations::EntityDerive),
                        #[codec(index = 10)]
                        SetAttributes(runtime_types::common::prov::operations::SetAttributes),
                        #[codec(index = 11)]
                        WasAssociatedWith(
                            runtime_types::common::prov::operations::WasAssociatedWith,
                        ),
                        #[codec(index = 12)]
                        WasAttributedTo(runtime_types::common::prov::operations::WasAttributedTo),
                        #[codec(index = 13)]
                        WasInformedBy(runtime_types::common::prov::operations::WasInformedBy),
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct CreateNamespace {
                        pub id: runtime_types::common::prov::id::NamespaceId,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub enum DerivationType {
                        #[codec(index = 0)]
                        None,
                        #[codec(index = 1)]
                        Revision,
                        #[codec(index = 2)]
                        Quotation,
                        #[codec(index = 3)]
                        PrimarySource,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct EndActivity {
                        pub namespace: runtime_types::common::prov::id::NamespaceId,
                        pub id: runtime_types::common::prov::id::ActivityId,
                        pub time: runtime_types::common::prov::operations::TimeWrapper,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct EntityDerive {
                        pub namespace: runtime_types::common::prov::id::NamespaceId,
                        pub id: runtime_types::common::prov::id::EntityId,
                        pub used_id: runtime_types::common::prov::id::EntityId,
                        pub activity_id:
                        ::core::option::Option<runtime_types::common::prov::id::ActivityId>,
                        pub typ: runtime_types::common::prov::operations::DerivationType,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct EntityExists {
                        pub namespace: runtime_types::common::prov::id::NamespaceId,
                        pub external_id: runtime_types::common::prov::id::ExternalId,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub enum SetAttributes {
                        #[codec(index = 0)]
                        Entity {
                            namespace: runtime_types::common::prov::id::NamespaceId,
                            id: runtime_types::common::prov::id::EntityId,
                            attributes: runtime_types::common::attributes::Attributes,
                        },
                        #[codec(index = 1)]
                        Agent {
                            namespace: runtime_types::common::prov::id::NamespaceId,
                            id: runtime_types::common::prov::id::AgentId,
                            attributes: runtime_types::common::attributes::Attributes,
                        },
                        #[codec(index = 2)]
                        Activity {
                            namespace: runtime_types::common::prov::id::NamespaceId,
                            id: runtime_types::common::prov::id::ActivityId,
                            attributes: runtime_types::common::attributes::Attributes,
                        },
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct StartActivity {
                        pub namespace: runtime_types::common::prov::id::NamespaceId,
                        pub id: runtime_types::common::prov::id::ActivityId,
                        pub time: runtime_types::common::prov::operations::TimeWrapper,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct TimeWrapper(pub ::core::primitive::i64, pub ::core::primitive::u32);

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct WasAssociatedWith {
                        pub id: runtime_types::common::prov::id::AssociationId,
                        pub role: ::core::option::Option<runtime_types::common::prov::id::Role>,
                        pub namespace: runtime_types::common::prov::id::NamespaceId,
                        pub activity_id: runtime_types::common::prov::id::ActivityId,
                        pub agent_id: runtime_types::common::prov::id::AgentId,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct WasAttributedTo {
                        pub id: runtime_types::common::prov::id::AttributionId,
                        pub role: ::core::option::Option<runtime_types::common::prov::id::Role>,
                        pub namespace: runtime_types::common::prov::id::NamespaceId,
                        pub entity_id: runtime_types::common::prov::id::EntityId,
                        pub agent_id: runtime_types::common::prov::id::AgentId,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct WasGeneratedBy {
                        pub namespace: runtime_types::common::prov::id::NamespaceId,
                        pub id: runtime_types::common::prov::id::EntityId,
                        pub activity: runtime_types::common::prov::id::ActivityId,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct WasInformedBy {
                        pub namespace: runtime_types::common::prov::id::NamespaceId,
                        pub activity: runtime_types::common::prov::id::ActivityId,
                        pub informing_activity: runtime_types::common::prov::id::ActivityId,
                    }
                }
            }
        }

        pub mod finality_grandpa {
            use super::runtime_types;

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct Equivocation<_0, _1, _2> {
                pub round_number: ::core::primitive::u64,
                pub identity: _0,
                pub first: (_1, _2),
                pub second: (_1, _2),
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct Precommit<_0, _1> {
                pub target_hash: _0,
                pub target_number: _1,
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct Prevote<_0, _1> {
                pub target_hash: _0,
                pub target_number: _1,
            }
        }

        pub mod frame_support {
            use super::runtime_types;

            pub mod dispatch {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub enum DispatchClass {
                    #[codec(index = 0)]
                    Normal,
                    #[codec(index = 1)]
                    Operational,
                    #[codec(index = 2)]
                    Mandatory,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct DispatchInfo {
                    pub weight: runtime_types::sp_weights::weight_v2::Weight,
                    pub class: runtime_types::frame_support::dispatch::DispatchClass,
                    pub pays_fee: runtime_types::frame_support::dispatch::Pays,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub enum Pays {
                    #[codec(index = 0)]
                    Yes,
                    #[codec(index = 1)]
                    No,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct PerDispatchClass<_0> {
                    pub normal: _0,
                    pub operational: _0,
                    pub mandatory: _0,
                }
            }
        }

        pub mod frame_system {
            use super::runtime_types;

            pub mod extensions {
                use super::runtime_types;

                pub mod check_genesis {
                    use super::runtime_types;

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct CheckGenesis;
                }

                pub mod check_mortality {
                    use super::runtime_types;

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct CheckMortality(pub runtime_types::sp_runtime::generic::era::Era);
                }

                pub mod check_non_zero_sender {
                    use super::runtime_types;

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct CheckNonZeroSender;
                }

                pub mod check_spec_version {
                    use super::runtime_types;

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct CheckSpecVersion;
                }

                pub mod check_tx_version {
                    use super::runtime_types;

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct CheckTxVersion;
                }

                pub mod check_weight {
                    use super::runtime_types;

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct CheckWeight;
                }
            }

            pub mod limits {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct BlockLength {
                    pub max: runtime_types::frame_support::dispatch::PerDispatchClass<
                        ::core::primitive::u32,
                    >,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct BlockWeights {
                    pub base_block: runtime_types::sp_weights::weight_v2::Weight,
                    pub max_block: runtime_types::sp_weights::weight_v2::Weight,
                    pub per_class: runtime_types::frame_support::dispatch::PerDispatchClass<
                        runtime_types::frame_system::limits::WeightsPerClass,
                    >,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct WeightsPerClass {
                    pub base_extrinsic: runtime_types::sp_weights::weight_v2::Weight,
                    pub max_extrinsic:
                    ::core::option::Option<runtime_types::sp_weights::weight_v2::Weight>,
                    pub max_total:
                    ::core::option::Option<runtime_types::sp_weights::weight_v2::Weight>,
                    pub reserved:
                    ::core::option::Option<runtime_types::sp_weights::weight_v2::Weight>,
                }
            }

            pub mod pallet {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "Contains a variant per dispatchable extrinsic that this pallet has."]
                pub enum Call {
                    #[codec(index = 0)]
                    #[doc = "See [`Pallet::remark`]."]
                    remark { remark: ::std::vec::Vec<::core::primitive::u8> },
                    #[codec(index = 1)]
                    #[doc = "See [`Pallet::set_heap_pages`]."]
                    set_heap_pages { pages: ::core::primitive::u64 },
                    #[codec(index = 2)]
                    #[doc = "See [`Pallet::set_code`]."]
                    set_code { code: ::std::vec::Vec<::core::primitive::u8> },
                    #[codec(index = 3)]
                    #[doc = "See [`Pallet::set_code_without_checks`]."]
                    set_code_without_checks { code: ::std::vec::Vec<::core::primitive::u8> },
                    #[codec(index = 4)]
                    #[doc = "See [`Pallet::set_storage`]."]
                    set_storage {
                        items: ::std::vec::Vec<(
                            ::std::vec::Vec<::core::primitive::u8>,
                            ::std::vec::Vec<::core::primitive::u8>,
                        )>,
                    },
                    #[codec(index = 5)]
                    #[doc = "See [`Pallet::kill_storage`]."]
                    kill_storage { keys: ::std::vec::Vec<::std::vec::Vec<::core::primitive::u8>> },
                    #[codec(index = 6)]
                    #[doc = "See [`Pallet::kill_prefix`]."]
                    kill_prefix {
                        prefix: ::std::vec::Vec<::core::primitive::u8>,
                        subkeys: ::core::primitive::u32,
                    },
                    #[codec(index = 7)]
                    #[doc = "See [`Pallet::remark_with_event`]."]
                    remark_with_event { remark: ::std::vec::Vec<::core::primitive::u8> },
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "Error for the System pallet"]
                pub enum Error {
                    #[codec(index = 0)]
                    #[doc = "The name of specification does not match between the current runtime"]
                    #[doc = "and the new runtime."]
                    InvalidSpecName,
                    #[codec(index = 1)]
                    #[doc = "The specification version is not allowed to decrease between the current runtime"]
                    #[doc = "and the new runtime."]
                    SpecVersionNeedsToIncrease,
                    #[codec(index = 2)]
                    #[doc = "Failed to extract the runtime version from the new runtime."]
                    #[doc = ""]
                    #[doc = "Either calling `Core_version` or decoding `RuntimeVersion` failed."]
                    FailedToExtractRuntimeVersion,
                    #[codec(index = 3)]
                    #[doc = "Suicide called when the account has non-default composite data."]
                    NonDefaultComposite,
                    #[codec(index = 4)]
                    #[doc = "There is a non-zero reference count preventing the account from being purged."]
                    NonZeroRefCount,
                    #[codec(index = 5)]
                    #[doc = "The origin filter prevent the call to be dispatched."]
                    CallFiltered,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "Event for the System pallet."]
                pub enum Event {
                    #[codec(index = 0)]
                    #[doc = "An extrinsic completed successfully."]
                    ExtrinsicSuccess {
                        dispatch_info: runtime_types::frame_support::dispatch::DispatchInfo,
                    },
                    #[codec(index = 1)]
                    #[doc = "An extrinsic failed."]
                    ExtrinsicFailed {
                        dispatch_error: runtime_types::sp_runtime::DispatchError,
                        dispatch_info: runtime_types::frame_support::dispatch::DispatchInfo,
                    },
                    #[codec(index = 2)]
                    #[doc = "`:code` was updated."]
                    CodeUpdated,
                    #[codec(index = 3)]
                    #[doc = "A new account was created."]
                    NewAccount { account: ::subxt::utils::AccountId32 },
                    #[codec(index = 4)]
                    #[doc = "An account was reaped."]
                    KilledAccount { account: ::subxt::utils::AccountId32 },
                    #[codec(index = 5)]
                    #[doc = "On on-chain remark happened."]
                    Remarked { sender: ::subxt::utils::AccountId32, hash: ::subxt::utils::H256 },
                }
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct AccountInfo<_0, _1> {
                pub nonce: _0,
                pub consumers: ::core::primitive::u32,
                pub providers: ::core::primitive::u32,
                pub sufficients: ::core::primitive::u32,
                pub data: _1,
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct EventRecord<_0, _1> {
                pub phase: runtime_types::frame_system::Phase,
                pub event: _0,
                pub topics: ::std::vec::Vec<_1>,
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct LastRuntimeUpgradeInfo {
                #[codec(compact)]
                pub spec_version: ::core::primitive::u32,
                pub spec_name: ::std::string::String,
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub enum Phase {
                #[codec(index = 0)]
                ApplyExtrinsic(::core::primitive::u32),
                #[codec(index = 1)]
                Finalization,
                #[codec(index = 2)]
                Initialization,
            }
        }

        pub mod pallet_chronicle {
            use super::runtime_types;

            pub mod pallet {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "Contains a variant per dispatchable extrinsic that this pallet has."]
                pub enum Call {
                    #[codec(index = 0)]
                    #[doc = "See [`Pallet::apply`]."]
                    apply { operations: runtime_types::common::ledger::OperationSubmission },
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "The `Error` enum of this pallet."]
                pub enum Error {
                    #[codec(index = 0)]
                    Address,
                    #[codec(index = 1)]
                    Contradiction,
                    #[codec(index = 2)]
                    Compaction,
                    #[codec(index = 3)]
                    Expansion,
                    #[codec(index = 4)]
                    Identity,
                    #[codec(index = 5)]
                    IRef,
                    #[codec(index = 6)]
                    NotAChronicleIri,
                    #[codec(index = 7)]
                    MissingId,
                    #[codec(index = 8)]
                    MissingProperty,
                    #[codec(index = 9)]
                    NotANode,
                    #[codec(index = 10)]
                    NotAnObject,
                    #[codec(index = 11)]
                    OpaExecutor,
                    #[codec(index = 12)]
                    SerdeJson,
                    #[codec(index = 13)]
                    SubmissionFormat,
                    #[codec(index = 14)]
                    Time,
                    #[codec(index = 15)]
                    Tokio,
                    #[codec(index = 16)]
                    Utf8,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "The `Event` enum of this pallet"]
                pub enum Event {
                    #[codec(index = 0)]
                    Applied(
                        runtime_types::common::prov::model::ProvModel,
                        runtime_types::common::identity::SignedIdentity,
                        [::core::primitive::u8; 16usize],
                    ),
                    #[codec(index = 1)]
                    Contradiction(
                        runtime_types::common::prov::model::contradiction::Contradiction,
                        runtime_types::common::identity::SignedIdentity,
                        [::core::primitive::u8; 16usize],
                    ),
                }
            }
        }

        pub mod pallet_grandpa {
            use super::runtime_types;

            pub mod pallet {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "Contains a variant per dispatchable extrinsic that this pallet has."]
                pub enum Call {
                    #[codec(index = 0)]
                    #[doc = "See [`Pallet::report_equivocation`]."]
                    report_equivocation {
                        equivocation_proof: ::std::boxed::Box<
                            runtime_types::sp_consensus_grandpa::EquivocationProof<
                                ::subxt::utils::H256,
                                ::core::primitive::u32,
                            >,
                        >,
                        key_owner_proof: runtime_types::sp_core::Void,
                    },
                    #[codec(index = 1)]
                    #[doc = "See [`Pallet::report_equivocation_unsigned`]."]
                    report_equivocation_unsigned {
                        equivocation_proof: ::std::boxed::Box<
                            runtime_types::sp_consensus_grandpa::EquivocationProof<
                                ::subxt::utils::H256,
                                ::core::primitive::u32,
                            >,
                        >,
                        key_owner_proof: runtime_types::sp_core::Void,
                    },
                    #[codec(index = 2)]
                    #[doc = "See [`Pallet::note_stalled`]."]
                    note_stalled {
                        delay: ::core::primitive::u32,
                        best_finalized_block_number: ::core::primitive::u32,
                    },
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "The `Error` enum of this pallet."]
                pub enum Error {
                    #[codec(index = 0)]
                    #[doc = "Attempt to signal GRANDPA pause when the authority set isn't live"]
                    #[doc = "(either paused or already pending pause)."]
                    PauseFailed,
                    #[codec(index = 1)]
                    #[doc = "Attempt to signal GRANDPA resume when the authority set isn't paused"]
                    #[doc = "(either live or already pending resume)."]
                    ResumeFailed,
                    #[codec(index = 2)]
                    #[doc = "Attempt to signal GRANDPA change with one already pending."]
                    ChangePending,
                    #[codec(index = 3)]
                    #[doc = "Cannot signal forced change so soon after last."]
                    TooSoon,
                    #[codec(index = 4)]
                    #[doc = "A key ownership proof provided as part of an equivocation report is invalid."]
                    InvalidKeyOwnershipProof,
                    #[codec(index = 5)]
                    #[doc = "An equivocation proof provided as part of an equivocation report is invalid."]
                    InvalidEquivocationProof,
                    #[codec(index = 6)]
                    #[doc = "A given equivocation report is valid but already previously reported."]
                    DuplicateOffenceReport,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "The `Event` enum of this pallet"]
                pub enum Event {
                    #[codec(index = 0)]
                    #[doc = "New authority set has been applied."]
                    NewAuthorities {
                        authority_set: ::std::vec::Vec<(
                            runtime_types::sp_consensus_grandpa::app::Public,
                            ::core::primitive::u64,
                        )>,
                    },
                    #[codec(index = 1)]
                    #[doc = "Current authority set has been paused."]
                    Paused,
                    #[codec(index = 2)]
                    #[doc = "Current authority set has been resumed."]
                    Resumed,
                }
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct StoredPendingChange<_0> {
                pub scheduled_at: _0,
                pub delay: _0,
                pub next_authorities:
                runtime_types::bounded_collections::weak_bounded_vec::WeakBoundedVec<(
                    runtime_types::sp_consensus_grandpa::app::Public,
                    ::core::primitive::u64,
                )>,
                pub forced: ::core::option::Option<_0>,
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub enum StoredState<_0> {
                #[codec(index = 0)]
                Live,
                #[codec(index = 1)]
                PendingPause { scheduled_at: _0, delay: _0 },
                #[codec(index = 2)]
                Paused,
                #[codec(index = 3)]
                PendingResume { scheduled_at: _0, delay: _0 },
            }
        }

        pub mod pallet_opa {
            use super::runtime_types;

            pub mod pallet {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "Contains a variant per dispatchable extrinsic that this pallet has."]
                pub enum Call {
                    #[codec(index = 0)]
                    #[doc = "See [`Pallet::apply`]."]
                    apply { submission: runtime_types::common::opa::core::codec::OpaSubmissionV1 },
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "The `Error` enum of this pallet."]
                pub enum Error {
                    #[codec(index = 0)]
                    OperationSignatureVerification,
                    #[codec(index = 1)]
                    InvalidSigningKey,
                    #[codec(index = 2)]
                    JsonSerialize,
                    #[codec(index = 3)]
                    InvalidOperation,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "The `Event` enum of this pallet"]
                pub enum Event {
                    #[codec(index = 0)]
                    PolicyUpdate(
                        runtime_types::common::opa::core::codec::PolicyMetaV1,
                        runtime_types::common::prov::model::ChronicleTransactionId,
                    ),
                    #[codec(index = 1)]
                    KeyUpdate(
                        runtime_types::common::opa::core::codec::KeysV1,
                        runtime_types::common::prov::model::ChronicleTransactionId,
                    ),
                }
            }
        }

        pub mod pallet_sudo {
            use super::runtime_types;

            pub mod pallet {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "Contains a variant per dispatchable extrinsic that this pallet has."]
                pub enum Call {
                    #[codec(index = 0)]
                    #[doc = "See [`Pallet::sudo`]."]
                    sudo { call: ::std::boxed::Box<runtime_types::runtime_chronicle::RuntimeCall> },
                    #[codec(index = 1)]
                    #[doc = "See [`Pallet::sudo_unchecked_weight`]."]
                    sudo_unchecked_weight {
                        call: ::std::boxed::Box<runtime_types::runtime_chronicle::RuntimeCall>,
                        weight: runtime_types::sp_weights::weight_v2::Weight,
                    },
                    #[codec(index = 2)]
                    #[doc = "See [`Pallet::set_key`]."]
                    set_key { new: ::subxt::utils::MultiAddress<::subxt::utils::AccountId32, ()> },
                    #[codec(index = 3)]
                    #[doc = "See [`Pallet::sudo_as`]."]
                    sudo_as {
                        who: ::subxt::utils::MultiAddress<::subxt::utils::AccountId32, ()>,
                        call: ::std::boxed::Box<runtime_types::runtime_chronicle::RuntimeCall>,
                    },
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "Error for the Sudo pallet"]
                pub enum Error {
                    #[codec(index = 0)]
                    #[doc = "Sender must be the Sudo account"]
                    RequireSudo,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "The `Event` enum of this pallet"]
                pub enum Event {
                    #[codec(index = 0)]
                    #[doc = "A sudo call just took place."]
                    Sudid {
                        sudo_result:
                        ::core::result::Result<(), runtime_types::sp_runtime::DispatchError>,
                    },
                    #[codec(index = 1)]
                    #[doc = "The sudo key has been updated."]
                    KeyChanged { old_sudoer: ::core::option::Option<::subxt::utils::AccountId32> },
                    #[codec(index = 2)]
                    #[doc = "A [sudo_as](Pallet::sudo_as) call just took place."]
                    SudoAsDone {
                        sudo_result:
                        ::core::result::Result<(), runtime_types::sp_runtime::DispatchError>,
                    },
                }
            }
        }

        pub mod pallet_timestamp {
            use super::runtime_types;

            pub mod pallet {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                #[doc = "Contains a variant per dispatchable extrinsic that this pallet has."]
                pub enum Call {
                    #[codec(index = 0)]
                    #[doc = "See [`Pallet::set`]."]
                    set {
                        #[codec(compact)]
                        now: ::core::primitive::u64,
                    },
                }
            }
        }

        pub mod runtime_chronicle {
            use super::runtime_types;

            pub mod no_nonce_fees {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct CheckNonce(#[codec(compact)] pub ::core::primitive::u32);
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct Runtime;

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub enum RuntimeCall {
                #[codec(index = 0)]
                System(runtime_types::frame_system::pallet::Call),
                #[codec(index = 1)]
                Timestamp(runtime_types::pallet_timestamp::pallet::Call),
                #[codec(index = 3)]
                Grandpa(runtime_types::pallet_grandpa::pallet::Call),
                #[codec(index = 4)]
                Sudo(runtime_types::pallet_sudo::pallet::Call),
                #[codec(index = 5)]
                Chronicle(runtime_types::pallet_chronicle::pallet::Call),
                #[codec(index = 6)]
                Opa(runtime_types::pallet_opa::pallet::Call),
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub enum RuntimeError {
                #[codec(index = 0)]
                System(runtime_types::frame_system::pallet::Error),
                #[codec(index = 3)]
                Grandpa(runtime_types::pallet_grandpa::pallet::Error),
                #[codec(index = 4)]
                Sudo(runtime_types::pallet_sudo::pallet::Error),
                #[codec(index = 5)]
                Chronicle(runtime_types::pallet_chronicle::pallet::Error),
                #[codec(index = 6)]
                Opa(runtime_types::pallet_opa::pallet::Error),
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub enum RuntimeEvent {
                #[codec(index = 0)]
                System(runtime_types::frame_system::pallet::Event),
                #[codec(index = 3)]
                Grandpa(runtime_types::pallet_grandpa::pallet::Event),
                #[codec(index = 4)]
                Sudo(runtime_types::pallet_sudo::pallet::Event),
                #[codec(index = 5)]
                Chronicle(runtime_types::pallet_chronicle::pallet::Event),
                #[codec(index = 6)]
                Opa(runtime_types::pallet_opa::pallet::Event),
            }
        }

        pub mod sp_arithmetic {
            use super::runtime_types;

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub enum ArithmeticError {
                #[codec(index = 0)]
                Underflow,
                #[codec(index = 1)]
                Overflow,
                #[codec(index = 2)]
                DivisionByZero,
            }
        }

        pub mod sp_consensus_aura {
            use super::runtime_types;

            pub mod sr25519 {
                use super::runtime_types;

                pub mod app_sr25519 {
                    use super::runtime_types;

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Public(pub runtime_types::sp_core::sr25519::Public);
                }
            }
        }

        pub mod sp_consensus_grandpa {
            use super::runtime_types;

            pub mod app {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct Public(pub runtime_types::sp_core::ed25519::Public);

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct Signature(pub runtime_types::sp_core::ed25519::Signature);
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub enum Equivocation<_0, _1> {
                #[codec(index = 0)]
                Prevote(
                    runtime_types::finality_grandpa::Equivocation<
                        runtime_types::sp_consensus_grandpa::app::Public,
                        runtime_types::finality_grandpa::Prevote<_0, _1>,
                        runtime_types::sp_consensus_grandpa::app::Signature,
                    >,
                ),
                #[codec(index = 1)]
                Precommit(
                    runtime_types::finality_grandpa::Equivocation<
                        runtime_types::sp_consensus_grandpa::app::Public,
                        runtime_types::finality_grandpa::Precommit<_0, _1>,
                        runtime_types::sp_consensus_grandpa::app::Signature,
                    >,
                ),
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct EquivocationProof<_0, _1> {
                pub set_id: ::core::primitive::u64,
                pub equivocation: runtime_types::sp_consensus_grandpa::Equivocation<_0, _1>,
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct OpaqueKeyOwnershipProof(pub ::std::vec::Vec<::core::primitive::u8>);
        }

        pub mod sp_consensus_slots {
            use super::runtime_types;

            #[derive(
                ::subxt::ext::codec::CompactAs,
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct Slot(pub ::core::primitive::u64);

            #[derive(
                ::subxt::ext::codec::CompactAs,
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct SlotDuration(pub ::core::primitive::u64);
        }

        pub mod sp_core {
            use super::runtime_types;

            pub mod crypto {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct KeyTypeId(pub [::core::primitive::u8; 4usize]);
            }

            pub mod ecdsa {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct Signature(pub [::core::primitive::u8; 65usize]);
            }

            pub mod ed25519 {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct Public(pub [::core::primitive::u8; 32usize]);

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct Signature(pub [::core::primitive::u8; 64usize]);
            }

            pub mod sr25519 {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct Public(pub [::core::primitive::u8; 32usize]);

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct Signature(pub [::core::primitive::u8; 64usize]);
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct OpaqueMetadata(pub ::std::vec::Vec<::core::primitive::u8>);

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub enum Void {}
        }

        pub mod sp_inherents {
            use super::runtime_types;

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct CheckInherentsResult {
                pub okay: ::core::primitive::bool,
                pub fatal_error: ::core::primitive::bool,
                pub errors: runtime_types::sp_inherents::InherentData,
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct InherentData {
                pub data: ::subxt::utils::KeyedVec<
                    [::core::primitive::u8; 8usize],
                    ::std::vec::Vec<::core::primitive::u8>,
                >,
            }
        }

        pub mod sp_runtime {
            use super::runtime_types;

            pub mod generic {
                use super::runtime_types;

                pub mod block {
                    use super::runtime_types;

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Block<_0, _1> {
                        pub header: _0,
                        pub extrinsics: ::std::vec::Vec<_1>,
                    }
                }

                pub mod digest {
                    use super::runtime_types;

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Digest {
                        pub logs:
                        ::std::vec::Vec<runtime_types::sp_runtime::generic::digest::DigestItem>,
                    }

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub enum DigestItem {
                        #[codec(index = 6)]
                        PreRuntime(
                            [::core::primitive::u8; 4usize],
                            ::std::vec::Vec<::core::primitive::u8>,
                        ),
                        #[codec(index = 4)]
                        Consensus(
                            [::core::primitive::u8; 4usize],
                            ::std::vec::Vec<::core::primitive::u8>,
                        ),
                        #[codec(index = 5)]
                        Seal(
                            [::core::primitive::u8; 4usize],
                            ::std::vec::Vec<::core::primitive::u8>,
                        ),
                        #[codec(index = 0)]
                        Other(::std::vec::Vec<::core::primitive::u8>),
                        #[codec(index = 8)]
                        RuntimeEnvironmentUpdated,
                    }
                }

                pub mod era {
                    use super::runtime_types;

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub enum Era {
                        #[codec(index = 0)]
                        Immortal,
                        #[codec(index = 1)]
                        Mortal1(::core::primitive::u8),
                        #[codec(index = 2)]
                        Mortal2(::core::primitive::u8),
                        #[codec(index = 3)]
                        Mortal3(::core::primitive::u8),
                        #[codec(index = 4)]
                        Mortal4(::core::primitive::u8),
                        #[codec(index = 5)]
                        Mortal5(::core::primitive::u8),
                        #[codec(index = 6)]
                        Mortal6(::core::primitive::u8),
                        #[codec(index = 7)]
                        Mortal7(::core::primitive::u8),
                        #[codec(index = 8)]
                        Mortal8(::core::primitive::u8),
                        #[codec(index = 9)]
                        Mortal9(::core::primitive::u8),
                        #[codec(index = 10)]
                        Mortal10(::core::primitive::u8),
                        #[codec(index = 11)]
                        Mortal11(::core::primitive::u8),
                        #[codec(index = 12)]
                        Mortal12(::core::primitive::u8),
                        #[codec(index = 13)]
                        Mortal13(::core::primitive::u8),
                        #[codec(index = 14)]
                        Mortal14(::core::primitive::u8),
                        #[codec(index = 15)]
                        Mortal15(::core::primitive::u8),
                        #[codec(index = 16)]
                        Mortal16(::core::primitive::u8),
                        #[codec(index = 17)]
                        Mortal17(::core::primitive::u8),
                        #[codec(index = 18)]
                        Mortal18(::core::primitive::u8),
                        #[codec(index = 19)]
                        Mortal19(::core::primitive::u8),
                        #[codec(index = 20)]
                        Mortal20(::core::primitive::u8),
                        #[codec(index = 21)]
                        Mortal21(::core::primitive::u8),
                        #[codec(index = 22)]
                        Mortal22(::core::primitive::u8),
                        #[codec(index = 23)]
                        Mortal23(::core::primitive::u8),
                        #[codec(index = 24)]
                        Mortal24(::core::primitive::u8),
                        #[codec(index = 25)]
                        Mortal25(::core::primitive::u8),
                        #[codec(index = 26)]
                        Mortal26(::core::primitive::u8),
                        #[codec(index = 27)]
                        Mortal27(::core::primitive::u8),
                        #[codec(index = 28)]
                        Mortal28(::core::primitive::u8),
                        #[codec(index = 29)]
                        Mortal29(::core::primitive::u8),
                        #[codec(index = 30)]
                        Mortal30(::core::primitive::u8),
                        #[codec(index = 31)]
                        Mortal31(::core::primitive::u8),
                        #[codec(index = 32)]
                        Mortal32(::core::primitive::u8),
                        #[codec(index = 33)]
                        Mortal33(::core::primitive::u8),
                        #[codec(index = 34)]
                        Mortal34(::core::primitive::u8),
                        #[codec(index = 35)]
                        Mortal35(::core::primitive::u8),
                        #[codec(index = 36)]
                        Mortal36(::core::primitive::u8),
                        #[codec(index = 37)]
                        Mortal37(::core::primitive::u8),
                        #[codec(index = 38)]
                        Mortal38(::core::primitive::u8),
                        #[codec(index = 39)]
                        Mortal39(::core::primitive::u8),
                        #[codec(index = 40)]
                        Mortal40(::core::primitive::u8),
                        #[codec(index = 41)]
                        Mortal41(::core::primitive::u8),
                        #[codec(index = 42)]
                        Mortal42(::core::primitive::u8),
                        #[codec(index = 43)]
                        Mortal43(::core::primitive::u8),
                        #[codec(index = 44)]
                        Mortal44(::core::primitive::u8),
                        #[codec(index = 45)]
                        Mortal45(::core::primitive::u8),
                        #[codec(index = 46)]
                        Mortal46(::core::primitive::u8),
                        #[codec(index = 47)]
                        Mortal47(::core::primitive::u8),
                        #[codec(index = 48)]
                        Mortal48(::core::primitive::u8),
                        #[codec(index = 49)]
                        Mortal49(::core::primitive::u8),
                        #[codec(index = 50)]
                        Mortal50(::core::primitive::u8),
                        #[codec(index = 51)]
                        Mortal51(::core::primitive::u8),
                        #[codec(index = 52)]
                        Mortal52(::core::primitive::u8),
                        #[codec(index = 53)]
                        Mortal53(::core::primitive::u8),
                        #[codec(index = 54)]
                        Mortal54(::core::primitive::u8),
                        #[codec(index = 55)]
                        Mortal55(::core::primitive::u8),
                        #[codec(index = 56)]
                        Mortal56(::core::primitive::u8),
                        #[codec(index = 57)]
                        Mortal57(::core::primitive::u8),
                        #[codec(index = 58)]
                        Mortal58(::core::primitive::u8),
                        #[codec(index = 59)]
                        Mortal59(::core::primitive::u8),
                        #[codec(index = 60)]
                        Mortal60(::core::primitive::u8),
                        #[codec(index = 61)]
                        Mortal61(::core::primitive::u8),
                        #[codec(index = 62)]
                        Mortal62(::core::primitive::u8),
                        #[codec(index = 63)]
                        Mortal63(::core::primitive::u8),
                        #[codec(index = 64)]
                        Mortal64(::core::primitive::u8),
                        #[codec(index = 65)]
                        Mortal65(::core::primitive::u8),
                        #[codec(index = 66)]
                        Mortal66(::core::primitive::u8),
                        #[codec(index = 67)]
                        Mortal67(::core::primitive::u8),
                        #[codec(index = 68)]
                        Mortal68(::core::primitive::u8),
                        #[codec(index = 69)]
                        Mortal69(::core::primitive::u8),
                        #[codec(index = 70)]
                        Mortal70(::core::primitive::u8),
                        #[codec(index = 71)]
                        Mortal71(::core::primitive::u8),
                        #[codec(index = 72)]
                        Mortal72(::core::primitive::u8),
                        #[codec(index = 73)]
                        Mortal73(::core::primitive::u8),
                        #[codec(index = 74)]
                        Mortal74(::core::primitive::u8),
                        #[codec(index = 75)]
                        Mortal75(::core::primitive::u8),
                        #[codec(index = 76)]
                        Mortal76(::core::primitive::u8),
                        #[codec(index = 77)]
                        Mortal77(::core::primitive::u8),
                        #[codec(index = 78)]
                        Mortal78(::core::primitive::u8),
                        #[codec(index = 79)]
                        Mortal79(::core::primitive::u8),
                        #[codec(index = 80)]
                        Mortal80(::core::primitive::u8),
                        #[codec(index = 81)]
                        Mortal81(::core::primitive::u8),
                        #[codec(index = 82)]
                        Mortal82(::core::primitive::u8),
                        #[codec(index = 83)]
                        Mortal83(::core::primitive::u8),
                        #[codec(index = 84)]
                        Mortal84(::core::primitive::u8),
                        #[codec(index = 85)]
                        Mortal85(::core::primitive::u8),
                        #[codec(index = 86)]
                        Mortal86(::core::primitive::u8),
                        #[codec(index = 87)]
                        Mortal87(::core::primitive::u8),
                        #[codec(index = 88)]
                        Mortal88(::core::primitive::u8),
                        #[codec(index = 89)]
                        Mortal89(::core::primitive::u8),
                        #[codec(index = 90)]
                        Mortal90(::core::primitive::u8),
                        #[codec(index = 91)]
                        Mortal91(::core::primitive::u8),
                        #[codec(index = 92)]
                        Mortal92(::core::primitive::u8),
                        #[codec(index = 93)]
                        Mortal93(::core::primitive::u8),
                        #[codec(index = 94)]
                        Mortal94(::core::primitive::u8),
                        #[codec(index = 95)]
                        Mortal95(::core::primitive::u8),
                        #[codec(index = 96)]
                        Mortal96(::core::primitive::u8),
                        #[codec(index = 97)]
                        Mortal97(::core::primitive::u8),
                        #[codec(index = 98)]
                        Mortal98(::core::primitive::u8),
                        #[codec(index = 99)]
                        Mortal99(::core::primitive::u8),
                        #[codec(index = 100)]
                        Mortal100(::core::primitive::u8),
                        #[codec(index = 101)]
                        Mortal101(::core::primitive::u8),
                        #[codec(index = 102)]
                        Mortal102(::core::primitive::u8),
                        #[codec(index = 103)]
                        Mortal103(::core::primitive::u8),
                        #[codec(index = 104)]
                        Mortal104(::core::primitive::u8),
                        #[codec(index = 105)]
                        Mortal105(::core::primitive::u8),
                        #[codec(index = 106)]
                        Mortal106(::core::primitive::u8),
                        #[codec(index = 107)]
                        Mortal107(::core::primitive::u8),
                        #[codec(index = 108)]
                        Mortal108(::core::primitive::u8),
                        #[codec(index = 109)]
                        Mortal109(::core::primitive::u8),
                        #[codec(index = 110)]
                        Mortal110(::core::primitive::u8),
                        #[codec(index = 111)]
                        Mortal111(::core::primitive::u8),
                        #[codec(index = 112)]
                        Mortal112(::core::primitive::u8),
                        #[codec(index = 113)]
                        Mortal113(::core::primitive::u8),
                        #[codec(index = 114)]
                        Mortal114(::core::primitive::u8),
                        #[codec(index = 115)]
                        Mortal115(::core::primitive::u8),
                        #[codec(index = 116)]
                        Mortal116(::core::primitive::u8),
                        #[codec(index = 117)]
                        Mortal117(::core::primitive::u8),
                        #[codec(index = 118)]
                        Mortal118(::core::primitive::u8),
                        #[codec(index = 119)]
                        Mortal119(::core::primitive::u8),
                        #[codec(index = 120)]
                        Mortal120(::core::primitive::u8),
                        #[codec(index = 121)]
                        Mortal121(::core::primitive::u8),
                        #[codec(index = 122)]
                        Mortal122(::core::primitive::u8),
                        #[codec(index = 123)]
                        Mortal123(::core::primitive::u8),
                        #[codec(index = 124)]
                        Mortal124(::core::primitive::u8),
                        #[codec(index = 125)]
                        Mortal125(::core::primitive::u8),
                        #[codec(index = 126)]
                        Mortal126(::core::primitive::u8),
                        #[codec(index = 127)]
                        Mortal127(::core::primitive::u8),
                        #[codec(index = 128)]
                        Mortal128(::core::primitive::u8),
                        #[codec(index = 129)]
                        Mortal129(::core::primitive::u8),
                        #[codec(index = 130)]
                        Mortal130(::core::primitive::u8),
                        #[codec(index = 131)]
                        Mortal131(::core::primitive::u8),
                        #[codec(index = 132)]
                        Mortal132(::core::primitive::u8),
                        #[codec(index = 133)]
                        Mortal133(::core::primitive::u8),
                        #[codec(index = 134)]
                        Mortal134(::core::primitive::u8),
                        #[codec(index = 135)]
                        Mortal135(::core::primitive::u8),
                        #[codec(index = 136)]
                        Mortal136(::core::primitive::u8),
                        #[codec(index = 137)]
                        Mortal137(::core::primitive::u8),
                        #[codec(index = 138)]
                        Mortal138(::core::primitive::u8),
                        #[codec(index = 139)]
                        Mortal139(::core::primitive::u8),
                        #[codec(index = 140)]
                        Mortal140(::core::primitive::u8),
                        #[codec(index = 141)]
                        Mortal141(::core::primitive::u8),
                        #[codec(index = 142)]
                        Mortal142(::core::primitive::u8),
                        #[codec(index = 143)]
                        Mortal143(::core::primitive::u8),
                        #[codec(index = 144)]
                        Mortal144(::core::primitive::u8),
                        #[codec(index = 145)]
                        Mortal145(::core::primitive::u8),
                        #[codec(index = 146)]
                        Mortal146(::core::primitive::u8),
                        #[codec(index = 147)]
                        Mortal147(::core::primitive::u8),
                        #[codec(index = 148)]
                        Mortal148(::core::primitive::u8),
                        #[codec(index = 149)]
                        Mortal149(::core::primitive::u8),
                        #[codec(index = 150)]
                        Mortal150(::core::primitive::u8),
                        #[codec(index = 151)]
                        Mortal151(::core::primitive::u8),
                        #[codec(index = 152)]
                        Mortal152(::core::primitive::u8),
                        #[codec(index = 153)]
                        Mortal153(::core::primitive::u8),
                        #[codec(index = 154)]
                        Mortal154(::core::primitive::u8),
                        #[codec(index = 155)]
                        Mortal155(::core::primitive::u8),
                        #[codec(index = 156)]
                        Mortal156(::core::primitive::u8),
                        #[codec(index = 157)]
                        Mortal157(::core::primitive::u8),
                        #[codec(index = 158)]
                        Mortal158(::core::primitive::u8),
                        #[codec(index = 159)]
                        Mortal159(::core::primitive::u8),
                        #[codec(index = 160)]
                        Mortal160(::core::primitive::u8),
                        #[codec(index = 161)]
                        Mortal161(::core::primitive::u8),
                        #[codec(index = 162)]
                        Mortal162(::core::primitive::u8),
                        #[codec(index = 163)]
                        Mortal163(::core::primitive::u8),
                        #[codec(index = 164)]
                        Mortal164(::core::primitive::u8),
                        #[codec(index = 165)]
                        Mortal165(::core::primitive::u8),
                        #[codec(index = 166)]
                        Mortal166(::core::primitive::u8),
                        #[codec(index = 167)]
                        Mortal167(::core::primitive::u8),
                        #[codec(index = 168)]
                        Mortal168(::core::primitive::u8),
                        #[codec(index = 169)]
                        Mortal169(::core::primitive::u8),
                        #[codec(index = 170)]
                        Mortal170(::core::primitive::u8),
                        #[codec(index = 171)]
                        Mortal171(::core::primitive::u8),
                        #[codec(index = 172)]
                        Mortal172(::core::primitive::u8),
                        #[codec(index = 173)]
                        Mortal173(::core::primitive::u8),
                        #[codec(index = 174)]
                        Mortal174(::core::primitive::u8),
                        #[codec(index = 175)]
                        Mortal175(::core::primitive::u8),
                        #[codec(index = 176)]
                        Mortal176(::core::primitive::u8),
                        #[codec(index = 177)]
                        Mortal177(::core::primitive::u8),
                        #[codec(index = 178)]
                        Mortal178(::core::primitive::u8),
                        #[codec(index = 179)]
                        Mortal179(::core::primitive::u8),
                        #[codec(index = 180)]
                        Mortal180(::core::primitive::u8),
                        #[codec(index = 181)]
                        Mortal181(::core::primitive::u8),
                        #[codec(index = 182)]
                        Mortal182(::core::primitive::u8),
                        #[codec(index = 183)]
                        Mortal183(::core::primitive::u8),
                        #[codec(index = 184)]
                        Mortal184(::core::primitive::u8),
                        #[codec(index = 185)]
                        Mortal185(::core::primitive::u8),
                        #[codec(index = 186)]
                        Mortal186(::core::primitive::u8),
                        #[codec(index = 187)]
                        Mortal187(::core::primitive::u8),
                        #[codec(index = 188)]
                        Mortal188(::core::primitive::u8),
                        #[codec(index = 189)]
                        Mortal189(::core::primitive::u8),
                        #[codec(index = 190)]
                        Mortal190(::core::primitive::u8),
                        #[codec(index = 191)]
                        Mortal191(::core::primitive::u8),
                        #[codec(index = 192)]
                        Mortal192(::core::primitive::u8),
                        #[codec(index = 193)]
                        Mortal193(::core::primitive::u8),
                        #[codec(index = 194)]
                        Mortal194(::core::primitive::u8),
                        #[codec(index = 195)]
                        Mortal195(::core::primitive::u8),
                        #[codec(index = 196)]
                        Mortal196(::core::primitive::u8),
                        #[codec(index = 197)]
                        Mortal197(::core::primitive::u8),
                        #[codec(index = 198)]
                        Mortal198(::core::primitive::u8),
                        #[codec(index = 199)]
                        Mortal199(::core::primitive::u8),
                        #[codec(index = 200)]
                        Mortal200(::core::primitive::u8),
                        #[codec(index = 201)]
                        Mortal201(::core::primitive::u8),
                        #[codec(index = 202)]
                        Mortal202(::core::primitive::u8),
                        #[codec(index = 203)]
                        Mortal203(::core::primitive::u8),
                        #[codec(index = 204)]
                        Mortal204(::core::primitive::u8),
                        #[codec(index = 205)]
                        Mortal205(::core::primitive::u8),
                        #[codec(index = 206)]
                        Mortal206(::core::primitive::u8),
                        #[codec(index = 207)]
                        Mortal207(::core::primitive::u8),
                        #[codec(index = 208)]
                        Mortal208(::core::primitive::u8),
                        #[codec(index = 209)]
                        Mortal209(::core::primitive::u8),
                        #[codec(index = 210)]
                        Mortal210(::core::primitive::u8),
                        #[codec(index = 211)]
                        Mortal211(::core::primitive::u8),
                        #[codec(index = 212)]
                        Mortal212(::core::primitive::u8),
                        #[codec(index = 213)]
                        Mortal213(::core::primitive::u8),
                        #[codec(index = 214)]
                        Mortal214(::core::primitive::u8),
                        #[codec(index = 215)]
                        Mortal215(::core::primitive::u8),
                        #[codec(index = 216)]
                        Mortal216(::core::primitive::u8),
                        #[codec(index = 217)]
                        Mortal217(::core::primitive::u8),
                        #[codec(index = 218)]
                        Mortal218(::core::primitive::u8),
                        #[codec(index = 219)]
                        Mortal219(::core::primitive::u8),
                        #[codec(index = 220)]
                        Mortal220(::core::primitive::u8),
                        #[codec(index = 221)]
                        Mortal221(::core::primitive::u8),
                        #[codec(index = 222)]
                        Mortal222(::core::primitive::u8),
                        #[codec(index = 223)]
                        Mortal223(::core::primitive::u8),
                        #[codec(index = 224)]
                        Mortal224(::core::primitive::u8),
                        #[codec(index = 225)]
                        Mortal225(::core::primitive::u8),
                        #[codec(index = 226)]
                        Mortal226(::core::primitive::u8),
                        #[codec(index = 227)]
                        Mortal227(::core::primitive::u8),
                        #[codec(index = 228)]
                        Mortal228(::core::primitive::u8),
                        #[codec(index = 229)]
                        Mortal229(::core::primitive::u8),
                        #[codec(index = 230)]
                        Mortal230(::core::primitive::u8),
                        #[codec(index = 231)]
                        Mortal231(::core::primitive::u8),
                        #[codec(index = 232)]
                        Mortal232(::core::primitive::u8),
                        #[codec(index = 233)]
                        Mortal233(::core::primitive::u8),
                        #[codec(index = 234)]
                        Mortal234(::core::primitive::u8),
                        #[codec(index = 235)]
                        Mortal235(::core::primitive::u8),
                        #[codec(index = 236)]
                        Mortal236(::core::primitive::u8),
                        #[codec(index = 237)]
                        Mortal237(::core::primitive::u8),
                        #[codec(index = 238)]
                        Mortal238(::core::primitive::u8),
                        #[codec(index = 239)]
                        Mortal239(::core::primitive::u8),
                        #[codec(index = 240)]
                        Mortal240(::core::primitive::u8),
                        #[codec(index = 241)]
                        Mortal241(::core::primitive::u8),
                        #[codec(index = 242)]
                        Mortal242(::core::primitive::u8),
                        #[codec(index = 243)]
                        Mortal243(::core::primitive::u8),
                        #[codec(index = 244)]
                        Mortal244(::core::primitive::u8),
                        #[codec(index = 245)]
                        Mortal245(::core::primitive::u8),
                        #[codec(index = 246)]
                        Mortal246(::core::primitive::u8),
                        #[codec(index = 247)]
                        Mortal247(::core::primitive::u8),
                        #[codec(index = 248)]
                        Mortal248(::core::primitive::u8),
                        #[codec(index = 249)]
                        Mortal249(::core::primitive::u8),
                        #[codec(index = 250)]
                        Mortal250(::core::primitive::u8),
                        #[codec(index = 251)]
                        Mortal251(::core::primitive::u8),
                        #[codec(index = 252)]
                        Mortal252(::core::primitive::u8),
                        #[codec(index = 253)]
                        Mortal253(::core::primitive::u8),
                        #[codec(index = 254)]
                        Mortal254(::core::primitive::u8),
                        #[codec(index = 255)]
                        Mortal255(::core::primitive::u8),
                    }
                }

                pub mod header {
                    use super::runtime_types;

                    #[derive(
                        ::subxt::ext::codec::Decode,
                        ::subxt::ext::codec::Encode,
                        ::subxt::ext::scale_decode::DecodeAsType,
                        ::subxt::ext::scale_encode::EncodeAsType,
                        Debug,
                    )]
                    #[codec(crate =::subxt::ext::codec)]
                    #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                    #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                    pub struct Header<_0> {
                        pub parent_hash: ::subxt::utils::H256,
                        #[codec(compact)]
                        pub number: _0,
                        pub state_root: ::subxt::utils::H256,
                        pub extrinsics_root: ::subxt::utils::H256,
                        pub digest: runtime_types::sp_runtime::generic::digest::Digest,
                    }
                }
            }

            pub mod transaction_validity {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub enum InvalidTransaction {
                    #[codec(index = 0)]
                    Call,
                    #[codec(index = 1)]
                    Payment,
                    #[codec(index = 2)]
                    Future,
                    #[codec(index = 3)]
                    Stale,
                    #[codec(index = 4)]
                    BadProof,
                    #[codec(index = 5)]
                    AncientBirthBlock,
                    #[codec(index = 6)]
                    ExhaustsResources,
                    #[codec(index = 7)]
                    Custom(::core::primitive::u8),
                    #[codec(index = 8)]
                    BadMandatory,
                    #[codec(index = 9)]
                    MandatoryValidation,
                    #[codec(index = 10)]
                    BadSigner,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub enum TransactionSource {
                    #[codec(index = 0)]
                    InBlock,
                    #[codec(index = 1)]
                    Local,
                    #[codec(index = 2)]
                    External,
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub enum TransactionValidityError {
                    #[codec(index = 0)]
                    Invalid(runtime_types::sp_runtime::transaction_validity::InvalidTransaction),
                    #[codec(index = 1)]
                    Unknown(runtime_types::sp_runtime::transaction_validity::UnknownTransaction),
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub enum UnknownTransaction {
                    #[codec(index = 0)]
                    CannotLookup,
                    #[codec(index = 1)]
                    NoUnsignedValidator,
                    #[codec(index = 2)]
                    Custom(::core::primitive::u8),
                }

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct ValidTransaction {
                    pub priority: ::core::primitive::u64,
                    pub requires: ::std::vec::Vec<::std::vec::Vec<::core::primitive::u8>>,
                    pub provides: ::std::vec::Vec<::std::vec::Vec<::core::primitive::u8>>,
                    pub longevity: ::core::primitive::u64,
                    pub propagate: ::core::primitive::bool,
                }
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub enum DispatchError {
                #[codec(index = 0)]
                Other,
                #[codec(index = 1)]
                CannotLookup,
                #[codec(index = 2)]
                BadOrigin,
                #[codec(index = 3)]
                Module(runtime_types::sp_runtime::ModuleError),
                #[codec(index = 4)]
                ConsumerRemaining,
                #[codec(index = 5)]
                NoProviders,
                #[codec(index = 6)]
                TooManyConsumers,
                #[codec(index = 7)]
                Token(runtime_types::sp_runtime::TokenError),
                #[codec(index = 8)]
                Arithmetic(runtime_types::sp_arithmetic::ArithmeticError),
                #[codec(index = 9)]
                Transactional(runtime_types::sp_runtime::TransactionalError),
                #[codec(index = 10)]
                Exhausted,
                #[codec(index = 11)]
                Corruption,
                #[codec(index = 12)]
                Unavailable,
                #[codec(index = 13)]
                RootNotAllowed,
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct ModuleError {
                pub index: ::core::primitive::u8,
                pub error: [::core::primitive::u8; 4usize],
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub enum MultiSignature {
                #[codec(index = 0)]
                Ed25519(runtime_types::sp_core::ed25519::Signature),
                #[codec(index = 1)]
                Sr25519(runtime_types::sp_core::sr25519::Signature),
                #[codec(index = 2)]
                Ecdsa(runtime_types::sp_core::ecdsa::Signature),
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub enum TokenError {
                #[codec(index = 0)]
                FundsUnavailable,
                #[codec(index = 1)]
                OnlyProvider,
                #[codec(index = 2)]
                BelowMinimum,
                #[codec(index = 3)]
                CannotCreate,
                #[codec(index = 4)]
                UnknownAsset,
                #[codec(index = 5)]
                Frozen,
                #[codec(index = 6)]
                Unsupported,
                #[codec(index = 7)]
                CannotCreateHold,
                #[codec(index = 8)]
                NotExpendable,
                #[codec(index = 9)]
                Blocked,
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub enum TransactionalError {
                #[codec(index = 0)]
                LimitReached,
                #[codec(index = 1)]
                NoLayer,
            }
        }

        pub mod sp_version {
            use super::runtime_types;

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct RuntimeVersion {
                pub spec_name: ::std::string::String,
                pub impl_name: ::std::string::String,
                pub authoring_version: ::core::primitive::u32,
                pub spec_version: ::core::primitive::u32,
                pub impl_version: ::core::primitive::u32,
                pub apis:
                ::std::vec::Vec<([::core::primitive::u8; 8usize], ::core::primitive::u32)>,
                pub transaction_version: ::core::primitive::u32,
                pub state_version: ::core::primitive::u8,
            }
        }

        pub mod sp_weights {
            use super::runtime_types;

            pub mod weight_v2 {
                use super::runtime_types;

                #[derive(
                    ::subxt::ext::codec::Decode,
                    ::subxt::ext::codec::Encode,
                    ::subxt::ext::scale_decode::DecodeAsType,
                    ::subxt::ext::scale_encode::EncodeAsType,
                    Debug,
                )]
                #[codec(crate =::subxt::ext::codec)]
                #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
                #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
                pub struct Weight {
                    #[codec(compact)]
                    pub ref_time: ::core::primitive::u64,
                    #[codec(compact)]
                    pub proof_size: ::core::primitive::u64,
                }
            }

            #[derive(
                ::subxt::ext::codec::Decode,
                ::subxt::ext::codec::Encode,
                ::subxt::ext::scale_decode::DecodeAsType,
                ::subxt::ext::scale_encode::EncodeAsType,
                Debug,
            )]
            #[codec(crate =::subxt::ext::codec)]
            #[decode_as_type(crate_path = ":: subxt :: ext :: scale_decode")]
            #[encode_as_type(crate_path = ":: subxt :: ext :: scale_encode")]
            pub struct RuntimeDbWeight {
                pub read: ::core::primitive::u64,
                pub write: ::core::primitive::u64,
            }
        }
    }
}
