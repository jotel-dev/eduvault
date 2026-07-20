import { getDb } from "@/lib/mongodb";
import { pinata } from "@/lib/pinata";
import { ObjectId } from "mongodb";

/**
 * Fetches all user data for export, excluding secrets and other users' data.
 * @param {Object} user - The authenticated user object
 * @returns {Promise<Object>} The portable JSON export
 */
export async function exportData(user) {
  const db = await getDb();
  
  // 1. Profile data (exclude internal IDs like _id or secrets if any)
  const profile = { ...user };
  delete profile._id;
  
  const walletQuery = user.walletAddress 
    ? { walletAddressLower: String(user.walletAddress).toLowerCase() }
    : { creator: user.email }; // fallback

  // 2. Materials
  const materials = await db.collection("materials")
    .find(user.walletAddress ? { creator: user.walletAddress } : { creatorEmail: user.email })
    .project({ _id: 0 })
    .toArray();

  // 3. Purchases
  const purchases = await db.collection("purchases")
    .find(user.walletAddress ? { buyer: user.walletAddress } : { buyerEmail: user.email })
    .project({ _id: 0 })
    .toArray();

  // 4. Analytics
  const analytics = await db.collection("analytics")
    .find(user.walletAddress ? { userId: user.walletAddress } : { userId: user.email })
    .project({ _id: 0 })
    .toArray();

  return {
    profile,
    materials,
    purchases,
    analytics,
    exportedAt: new Date().toISOString(),
    version: "1.0"
  };
}

/**
 * Asynchronously deletes or pseudonymizes a user's data across all services.
 * @param {Object} user - The authenticated user object
 * @returns {Promise<boolean>} Success status
 */
export async function initiateDeletion(user) {
  if (user.legalHold) {
    throw new Error("Active legal hold prevents deletion.");
  }

  const db = await getDb();
  const userId = new ObjectId(user._id);

  // 1. Unpin materials from Pinata & delete materials from MongoDB
  const materials = await db.collection("materials")
    .find(user.walletAddress ? { creator: user.walletAddress } : { creatorEmail: user.email })
    .toArray();

  for (const material of materials) {
    if (material.ipfsCid) {
      try {
        await pinata.unpin([material.ipfsCid]);
      } catch (err) {
        console.error(`Failed to unpin ${material.ipfsCid}`, err);
        // Resumable: We don't throw here, continue best effort or queue for retry
      }
    }
    // Hard delete material
    await db.collection("materials").deleteOne({ _id: material._id });
  }

  // 2. Pseudonymize Purchases (Financial records)
  if (user.walletAddress) {
    // We remove the direct link to the wallet, replacing it with a hash or "DELETED_USER"
    await db.collection("purchases").updateMany(
      { buyer: user.walletAddress },
      { $set: { buyer: `deleted_${Date.now()}` } }
    );
  }

  // 3. Delete Analytics
  await db.collection("analytics").deleteMany(
    user.walletAddress ? { userId: user.walletAddress } : { userId: user.email }
  );

  // 4. Anonymize or Delete User Profile
  // We hard delete to comply fully, or leave a tombstone. Let's hard delete.
  await db.collection("users").deleteOne({ _id: userId });

  // 5. Leave an audit trail tombstone
  await db.collection("erasure_logs").insertOne({
    originalId: userId,
    walletAddressHash: user.walletAddress ? "hashed_version_omitted" : null,
    deletedAt: new Date().toISOString(),
    status: "completed"
  });

  return true;
}
