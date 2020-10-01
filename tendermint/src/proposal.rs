//! Proposals from validators

mod canonical_proposal;
mod msg_type;
mod sign_proposal;

pub use self::canonical_proposal::CanonicalProposal;
pub use msg_type::Type;
pub use sign_proposal::{SignProposalRequest, SignedProposalResponse};

use crate::block::{Height, Id as BlockId, Round};
use crate::chain::Id as ChainId;
use crate::consensus::State;
use crate::Signature;
use crate::Time;
use crate::{Error, Kind};
use bytes::BufMut;
use std::convert::{TryFrom, TryInto};
use tendermint_proto::types::Proposal as RawProposal;
use tendermint_proto::{DomainType, Error as DomainTypeError};

/// Proposal
#[derive(Clone, PartialEq, Debug)]
pub struct Proposal {
    /// Proposal message type
    pub msg_type: Type,
    /// Height
    pub height: Height,
    /// Round
    pub round: Round,
    /// POL Round
    pub pol_round: Option<Round>,
    /// Block ID
    pub block_id: Option<BlockId>,
    /// Timestamp
    pub timestamp: Option<Time>,
    /// Signature
    pub signature: Signature,
}

impl DomainType<RawProposal> for Proposal {}

impl TryFrom<RawProposal> for Proposal {
    type Error = Error;

    fn try_from(value: RawProposal) -> Result<Self, Self::Error> {
        if value.pol_round < -1 {
            return Err(Kind::NegativePOLRound.into());
        }
        let pol_round = match value.pol_round {
            -1 => None,
            n => Some(Round::try_from(n)?),
        };
        Ok(Proposal {
            msg_type: value.r#type.try_into()?,
            height: value.height.try_into()?,
            round: value.round.try_into()?,
            pol_round,
            block_id: match value.block_id {
                None => None,
                Some(raw_block_id) => Some(BlockId::try_from(raw_block_id).unwrap()),
            },
            timestamp: match value.timestamp {
                None => None,
                Some(t) => Some(t.try_into()?),
            },
            signature: value.signature.try_into()?,
        })
    }
}

impl From<Proposal> for RawProposal {
    fn from(value: Proposal) -> Self {
        RawProposal {
            r#type: value.msg_type.into(),
            height: value.height.into(),
            round: value.round.into(),
            pol_round: value.pol_round.map_or(-1, |p| p.into()),
            block_id: value.block_id.map(|b| b.into()),
            timestamp: value.timestamp.map(|t| t.into()),
            signature: value.signature.into(),
        }
    }
}

impl Proposal {
    /// Create signable bytes from Proposal.
    pub fn to_signable_bytes<B>(
        &self,
        chain_id: ChainId,
        sign_bytes: &mut B,
    ) -> Result<bool, DomainTypeError>
    where
        B: BufMut,
    {
        CanonicalProposal::new(self.clone(), chain_id).encode_length_delimited(sign_bytes)?;
        Ok(true)
    }

    /// Create signable vector from Proposal.
    pub fn to_signable_vec(&self, chain_id: ChainId) -> Result<Vec<u8>, DomainTypeError> {
        CanonicalProposal::new(self.clone(), chain_id).encode_length_delimited_vec()
    }

    /// Consensus state from this proposal - This doesn't seem to be used anywhere.
    #[deprecated(
        since = "0.17.0",
        note = "This seems unnecessary, please raise it to the team, if you need it."
    )]
    pub fn consensus_state(&self) -> State {
        State {
            height: self.height,
            round: self.round,
            step: 3,
            block_id: self.block_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::block::parts::Header;
    use crate::block::Id as BlockId;
    use crate::block::{Height, Round};
    use crate::chain::Id as ChainId;
    use crate::hash::{Algorithm, Hash};
    use crate::proposal::SignProposalRequest;
    use crate::signature::{Ed25519Signature, ED25519_SIGNATURE_SIZE};
    use crate::{proposal::Type, Proposal, Signature};
    use chrono::{DateTime, Utc};
    use std::convert::TryFrom;
    use std::str::FromStr;
    use tendermint_proto::DomainType;

    #[test]
    fn test_serialization() {
        let dt = "2018-02-11T07:09:22.765Z".parse::<DateTime<Utc>>().unwrap();
        let proposal = Proposal {
            msg_type: Type::Proposal,
            height: Height::try_from(12345_i64).unwrap(),
            round: Round::try_from(23456).unwrap(),
            pol_round: None,
            block_id: Some(BlockId {
                hash: Hash::from_hex_upper(Algorithm::Sha256, "DEADBEEFDEADBEEFBAFBAFBAFBAFBAFA")
                    .unwrap(),
                parts: Some(Header {
                    total: 65535,
                    hash: Hash::from_hex_upper(
                        Algorithm::Sha256,
                        "0022446688AACCEE1133557799BBDDFF",
                    )
                    .unwrap(),
                }),
            }),
            timestamp: Some(dt.into()),
            signature: Signature::Ed25519(Ed25519Signature::new([0; ED25519_SIGNATURE_SIZE])),
        };

        let mut got = vec![];

        let request = SignProposalRequest {
            proposal,
            chain_id: ChainId::from_str("test_chain_id").unwrap(),
        };

        let _have = request.to_signable_bytes(&mut got);

        // the following vector is generated via:
        /*
           import (
               "fmt"
               prototypes "github.com/tendermint/tendermint/proto/tendermint/types"
               "github.com/tendermint/tendermint/types"
               "strings"
               "time"
           )
           func proposalSerialize() {
               stamp, _ := time.Parse(time.RFC3339Nano, "2018-02-11T07:09:22.765Z")
               proposal := &types.Proposal{
                   Type:     prototypes.SignedMsgType(prototypes.ProposalType),
                   Height:   12345,
                   Round:    23456,
                   POLRound: -1,
                   BlockID: types.BlockID{
                       Hash: []byte("DEADBEEFDEADBEEFBAFBAFBAFBAFBAFA"),
                       PartSetHeader: types.PartSetHeader{
                           Hash:  []byte("0022446688AACCEE1133557799BBDDFF"),
                           Total: 65535,
                       },
                   },
                   Timestamp: stamp,
               }
               signBytes := types.ProposalSignBytes("test_chain_id",proposal.ToProto())
               fmt.Println(strings.Join(strings.Split(fmt.Sprintf("%v", signBytes), " "), ", "))
           }
        */

        let want = vec![
            136, 1, 8, 32, 17, 57, 48, 0, 0, 0, 0, 0, 0, 25, 160, 91, 0, 0, 0, 0, 0, 0, 32, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 1, 42, 74, 10, 32, 68, 69, 65, 68, 66, 69, 69,
            70, 68, 69, 65, 68, 66, 69, 69, 70, 66, 65, 70, 66, 65, 70, 66, 65, 70, 66, 65, 70, 66,
            65, 70, 65, 18, 38, 8, 255, 255, 3, 18, 32, 48, 48, 50, 50, 52, 52, 54, 54, 56, 56, 65,
            65, 67, 67, 69, 69, 49, 49, 51, 51, 53, 53, 55, 55, 57, 57, 66, 66, 68, 68, 70, 70, 50,
            12, 8, 162, 216, 255, 211, 5, 16, 192, 242, 227, 236, 2, 58, 13, 116, 101, 115, 116,
            95, 99, 104, 97, 105, 110, 95, 105, 100,
        ];

        assert_eq!(got, want) // Todo: this fails, fix it before merging (want is correct)
    }

    #[test]
    fn test_deserialization() {
        let dt = "2018-02-11T07:09:22.765Z".parse::<DateTime<Utc>>().unwrap();
        let proposal = Proposal {
            msg_type: Type::Proposal,
            height: Height::try_from(12345_u64).unwrap(),
            round: Round::try_from(23456).unwrap(),
            timestamp: Some(dt.into()),

            pol_round: None,
            block_id: Some(BlockId {
                hash: Hash::from_hex_upper(Algorithm::Sha256, "DEADBEEFDEADBEEFBAFBAFBAFBAFBAFA")
                    .unwrap(),
                parts: Some(Header {
                    total: 65535,
                    hash: Hash::from_hex_upper(
                        Algorithm::Sha256,
                        "0022446688AACCEE1133557799BBDDFF",
                    )
                    .unwrap(),
                }),
            }),
            signature: Signature::Ed25519(Ed25519Signature::new([0; ED25519_SIGNATURE_SIZE])),
        };
        let want = SignProposalRequest {
            proposal,
            chain_id: ChainId::from_str("test_chain_id").unwrap(),
        };

        let data = vec![
            10, 110, 8, 32, 16, 185, 96, 24, 160, 183, 1, 32, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 1, 42, 74, 10, 32, 68, 69, 65, 68, 66, 69, 69, 70, 68, 69, 65, 68, 66, 69,
            69, 70, 66, 65, 70, 66, 65, 70, 66, 65, 70, 66, 65, 70, 66, 65, 70, 65, 18, 38, 8, 255,
            255, 3, 18, 32, 48, 48, 50, 50, 52, 52, 54, 54, 56, 56, 65, 65, 67, 67, 69, 69, 49, 49,
            51, 51, 53, 53, 55, 55, 57, 57, 66, 66, 68, 68, 70, 70, 50, 12, 8, 162, 216, 255, 211,
            5, 16, 192, 242, 227, 236, 2, 18, 13, 116, 101, 115, 116, 95, 99, 104, 97, 105, 110,
            95, 105, 100,
        ];

        let have = SignProposalRequest::decode_vec(&data).unwrap();
        assert_eq!(have, want); // Todo: this fails, fix before merging. Possibly data is bad.
    }
}
