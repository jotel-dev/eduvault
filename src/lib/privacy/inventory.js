/**
 * Privacy Data Inventory
 * Maps personal fields to purpose, lawful basis, owner, storage, retention, and deletion method.
 */
export const dataInventory = {
  profile: {
    fullName: {
      purpose: "Display name and account personalization",
      lawfulBasis: "Consent",
      owner: "User",
      storage: "MongoDB (users collection)",
      retention: "Until account deletion",
      deletionMethod: "Anonymize (replaced with 'Deleted User')",
    },
    email: {
      purpose: "Authentication and notifications",
      lawfulBasis: "Contractual necessity",
      owner: "User",
      storage: "MongoDB (users collection)",
      retention: "Until account deletion",
      deletionMethod: "Hard delete",
    },
    walletAddress: {
      purpose: "Decentralized identity and payments",
      lawfulBasis: "Contractual necessity",
      owner: "User",
      storage: "MongoDB (users collection)",
      retention: "Until account deletion",
      deletionMethod: "Anonymize (replaced with hash/null)",
    },
  },
  materials: {
    content: {
      purpose: "Educational material delivery",
      lawfulBasis: "Contractual necessity",
      owner: "Creator",
      storage: "IPFS via Pinata",
      retention: "Until deleted by creator or platform policy",
      deletionMethod: "Unpin from Pinata, cryptographic erasure if encrypted",
    },
    metadata: {
      purpose: "Catalog search and indexing",
      lawfulBasis: "Legitimate interest",
      owner: "Creator",
      storage: "MongoDB (materials collection)",
      retention: "Until deleted by creator",
      deletionMethod: "Hard delete / Unpin from Pinata",
    },
  },
  financial: {
    purchases: {
      purpose: "Proof of entitlement and royalty distribution",
      lawfulBasis: "Legal obligation (financial records)",
      owner: "Platform",
      storage: "Soroban Smart Contracts & MongoDB (purchases)",
      retention: "Immutable on-chain, 7 years off-chain",
      deletionMethod: "Pseudonymization off-chain (wallet unlinked)",
    },
  },
  analytics: {
    events: {
      purpose: "Platform usage and performance monitoring",
      lawfulBasis: "Legitimate interest",
      owner: "Platform",
      storage: "MongoDB (analytics collection)",
      retention: "1 year",
      deletionMethod: "Hard delete / Aggregation",
    },
  },
};

export function getInventory() {
  return dataInventory;
}
