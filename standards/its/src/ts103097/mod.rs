extern crate alloc;
use alloc::string::ToString;
/// ASN.1 definitions for ETSI TS 103 097 extension module
pub mod extension_module;
use crate::ieee1609dot2::{Certificate, CertificateId, HashedData, Ieee1609Dot2Data};

use rasn::error::InnerSubtypeConstraintError;
use rasn::prelude::*;

pub const ETSI_TS103097_MODULE_OID: &Oid = Oid::const_new(&[0, 4, 0, 5, 5, 103_097, 1, 3, 1]);

/// ETSI TS 103 097 certificate
#[derive(Debug, Clone, AsnType, Encode, Decode, PartialEq, Eq)]
#[rasn(delegate)]
pub struct EtsiTs103097Certificate(Certificate);

impl core::ops::Deref for EtsiTs103097Certificate {
    type Target = Certificate;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<Certificate> for EtsiTs103097Certificate {
    type Error = rasn::error::InnerSubtypeConstraintError;
    fn try_from(cert: Certificate) -> Result<Self, Self::Error> {
        let etsi_cert = EtsiTs103097Certificate(cert);
        etsi_cert.validate_components()
    }
}

impl InnerSubtypeConstraint for EtsiTs103097Certificate {
    fn validate_components(self) -> Result<Self, rasn::error::InnerSubtypeConstraintError> {
        let tbs = &self.0.to_be_signed;
        let id = &tbs.id;

        if !matches!(id, CertificateId::Name(_)) {
            return Err(
                rasn::error::InnerSubtypeConstraintError::InvalidComponentValue {
                    type_name: "EtsiTs103097Certificate::toBeSignedCertificate::CertificateId",
                    component_name: "CertificateId",
                    details: "Only CertificateId::Name is permitted".to_string(),
                },
            );
        }
        if tbs.cert_request_permissions.is_some() {
            return Err(
                rasn::error::InnerSubtypeConstraintError::UnexpectedComponentPresent {
                    type_name: "EtsiTs103097Certificate::toBeSignedCertificate",
                    component_name: "certRequestPermissions",
                },
            );
        }
        if tbs.can_request_rollover.is_some() {
            return Err(
                rasn::error::InnerSubtypeConstraintError::UnexpectedComponentPresent {
                    type_name: "EtsiTs103097Certificate::toBeSignedCertificate",
                    component_name: "canRequestRollover",
                },
            );
        }
        Ok(self)
    }
}

// #[derive(InnerSubtypeConstraint)]
// #[inner_subtype_constraint(
//     content(
//         signedData(
//             tbsData(
//                 headerInfo(present = "generationTime", absent = ["p2pcdLearningRequest", "missingCrlIdentifier"])
//             ),
//             signer(
//                 certificate(constrained = "EtsiTs103097Certificate", size = 1)
//             )
//         ),
//         encryptedData(
//             recipients(absent = ["pskRecipInfo", "symmRecipInfo", "rekRecipInfo"])
//         ),
//         absent = "signedCertificateRequest"
//     )
// )]
// #[derive(AsnType, Debug, Decode, Encode, PartialEq)]
// pub struct EtsiTs103097Data(Ieee1609Dot2Data);
//

// #[inner_subtype_constraint(
//     content(
//         signedData(
//             choice = true,
//             tbsData(
//                 headerInfo(present = "generationTime", absent = ["p2pcdLearningRequest", "missingCrlIdentifier"])
//             ),
//             signer(
//                 certificate(constrained = "EtsiTs103097Certificate", size = 1)
//             )
//         ),
//         encryptedData(
//             recipients(absent = ["pskRecipInfo", "symmRecipInfo", "rekRecipInfo"])
//         ),
//         absent = "signedCertificateRequest"
//     )
// )]
// #[derive(AsnType, Debug, Decode, Encode, PartialEq, InnerSubtypeConstraint)]
// #[inner_subtype_constraint(
//     content => {
//         kind = "choice:Ieee1609Dot2Content",
//         signedData => {
//             kind = "sequence:SignedData",
//             tbsData => {
//                 headerInfo => {
//                     generationTime => present,
//                     p2pcdLearningRequest => absent,
//                     missingCrlIdentifier => absent
//                 }
//             },
//             signer => {
//                 certificate => (EtsiTs103097Certificate, size(1))
//             }
//         },
//         encryptedData => {
//             recipients => {
//                 pskRecipInfo => absent,
//                 symmRecipInfo=> absent,
//                 rekRecipInfo => absent
//             }
//         },
//         signedCertificateRequest => absent
//     },
// )]
// #[inner_subtype_constraint(
//     target = "EtsiTs103097Data",
//     content => (
//         {
//             kind = "choice:Ieee1609Dot2Content",
//             signedData => {
//                 kind = "sequence:SignedData",
//                 tbsData => {
//                     headerInfo => {
//                         generationTime => present,
//                         p2pcdLearningRequest => absent,
//                         missingCrlIdentifier => absent
//                     }
//                 },
//                 signer => {
//                     certificate => (EtsiTs103097Certificate, size(1))
//                 }
//             }
//         } or {
//             kind = "choice:Ieee1609Dot2Content",
//             encryptedData => {
//                 recipients => {
//                     pskRecipInfo => absent,
//                     symmRecipInfo => absent,
//                     rekRecipInfo => absent
//                 }
//             }
//         } or signedCertificateRequest => absent
//     )
// )]
// pub struct EtsiTs103097Data(Ieee1609Dot2Data);
//
// SignedDataPayload ::= SEQUENCE {
//   data        Ieee1609Dot2Data OPTIONAL,
//   extDataHash HashedData OPTIONAL,
//   ...,
//   omitted     NULL OPTIONAL
// } (WITH COMPONENTS {..., data PRESENT} |
//    WITH COMPONENTS {..., extDataHash PRESENT} |
//    WITH COMPONENTS {..., omitted PRESENT})
// #[derive(AsnType, Debug, Clone, Decode, Encode, PartialEq, Eq, Hash, InnerSubtypeConstraint)]
// #[inner_subtype_constraint(
//     (data => present) or (extDataHash => present) or (omitted => present)
// )]
// #[rasn(automatic_tags)]
// #[non_exhaustive]
// pub struct SignedDataPayload2 {
//     pub data: Option<Ieee1609Dot2Data>,
//     #[rasn(identifier = "extDataHash")]
//     pub ext_data_hash: Option<HashedData>,
//     #[rasn(extension_addition)]
//     pub omitted: Option<()>,
// }

// #[derive(AsnType, Debug, Clone, Decode, Encode, PartialEq, Eq, Hash)]
// #[rasn(automatic_tags)]
//         let dsl = quote! {
// r#type => choice(CertificateType::implicit),
// toBeSigned => {
//     verifyKeyIndicator => choice(VerificationKeyIndicator::reconstructionValue)
// },
// signature => absent
// };
// pub struct CertificateBase2 {
//     #[rasn(value("3"))]
//     #[builder(default = CertificateBase::VERSION)]
//     pub version: Uint8,
//     #[rasn(identifier = "type")]
//     pub r#type: CertificateType,
//     pub issuer: IssuerIdentifier,
//     #[rasn(identifier = "toBeSigned")]
//     pub to_be_signed: ToBeSignedCertificate,
//     pub signature: Option<Signature>,
// }
// impl CertificateBase2 {
//     pub const VERSION: u8 = 3;
//     #[must_use]
//     pub const fn is_implicit(&self) -> bool {
//         matches!(
//             &self,
//              // self.0 instead
//             CertificateBase {
//                 r#type: CertificateType::Implicit,
//                 to_be_signed: ToBeSignedCertificate {
//                     verify_key_indicator: VerificationKeyIndicator::ReconstructionValue(_),
//                     ..
//                 },
//                 signature: None,
//                 ..
//             }
//         )
//     }
//     #[must_use]
//     pub const fn is_explicit(&self) -> bool {
//         matches!(
//             self,
//             CertificateBase {
//                 r#type: CertificateType::Explicit,
//                 to_be_signed: ToBeSignedCertificate {
//                     verify_key_indicator: VerificationKeyIndicator::VerificationKey(_),
//                     ..
//                 },
//                 signature: Some(_),
//                 ..
//             }
//         )
//     }
// }
