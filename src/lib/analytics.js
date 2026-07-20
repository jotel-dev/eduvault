import { getDb } from "@/lib/mongodb";

/**
 * A basic analytics tracking utility to demonstrate the privacy workflow.
 * Analytics records will be associated with users but designed to be easily 
 * anonymized or deleted during the erasure process.
 */

export async function trackEvent(userId, eventName, metadata = {}) {
  try {
    const db = await getDb();
    const event = {
      userId,
      eventName,
      metadata,
      timestamp: new Date().toISOString()
    };
    await db.collection("analytics").insertOne(event);
  } catch (error) {
    console.error("Failed to track analytics event:", error);
    // Fail silently so it doesn't disrupt user flow
  }
}
