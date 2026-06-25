import { MongoClient } from "mongodb";

const uri = process.env.MONGODB_URI;

const globalForMongo = globalThis;

function getClientPromise() {
  if (!uri) {
    throw new Error("MONGODB_URI is not set in environment variables");
  }

  // Reuse the client across hot reloads in dev, but only connect on demand.
  if (!globalForMongo._mongoClientPromise) {
    const client = new MongoClient(uri);
    globalForMongo._mongoClientPromise = client.connect();
  }

  return globalForMongo._mongoClientPromise;
}

let indexesCreated = false;

async function ensureIndexes(db) {
  try {
    const collection = db.collection("materials");
    
    // Create compound index for category and price search optimization
    await collection.createIndex(
      { category: 1, price: 1 },
      { name: "materials_category_price_idx", background: true }
    );

    // Create compound text index for title and description search
    await collection.createIndex(
      { title: "text", description: "text" },
      { name: "materials_text_idx", background: true }
    );

    // Create compound index for title, description, price, and category
    await collection.createIndex(
      { category: 1, price: 1, title: 1, description: 1 },
      { name: "materials_search_compound_idx", background: true }
    );

    console.log("MongoDB indexes ensured successfully.");
  } catch (error) {
    console.error("Failed to create MongoDB indexes:", error);
  }
}

export async function getDb() {
  const client = await getClientPromise();
  // When DB name is in connection string, driver selects it automatically.
  // Otherwise, fallback to "eduvault".
  const dbName = process.env.MONGODB_DB || "eduvault";
  const db = client.db(dbName);

  if (!indexesCreated) {
    indexesCreated = true;
    ensureIndexes(db).catch(err => console.error("Error creating indexes:", err));
  }

  return db;
}
