import { describe, it, expect, vi, beforeEach } from 'vitest';
import { exportData, initiateDeletion } from '@/lib/privacy/erasure';
import { getInventory } from '@/lib/privacy/inventory';
import { getDb } from '@/lib/mongodb';
import { pinata } from '@/lib/pinata';

// Mock dependencies
vi.mock('@/lib/mongodb', () => {
  const mockDb = {
    collection: vi.fn(),
  };
  return { getDb: vi.fn(() => mockDb) };
});

vi.mock('@/lib/pinata', () => {
  return {
    pinata: {
      unpin: vi.fn(),
    },
  };
});

describe('Privacy Data Lifecycle', () => {
  let mockCollection;

  beforeEach(() => {
    vi.clearAllMocks();
    mockCollection = {
      find: vi.fn().mockReturnThis(),
      project: vi.fn().mockReturnThis(),
      toArray: vi.fn().mockResolvedValue([]),
      deleteOne: vi.fn().mockResolvedValue({}),
      deleteMany: vi.fn().mockResolvedValue({}),
      updateMany: vi.fn().mockResolvedValue({}),
      insertOne: vi.fn().mockResolvedValue({}),
    };

    getDb.mockResolvedValue({
      collection: vi.fn((name) => mockCollection),
    });
  });

  describe('Inventory', () => {
    it('returns a well-formed machine-readable JSON inventory', () => {
      const inventory = getInventory();
      expect(inventory).toBeDefined();
      expect(inventory.profile.email.lawfulBasis).toBe('Contractual necessity');
      expect(inventory.financial.purchases.deletionMethod).toContain('Pseudonymization');
    });
  });

  describe('exportData', () => {
    it('exports all user data excluding internal secrets', async () => {
      const user = { _id: '123', email: 'test@test.com', fullName: 'Test User' };
      
      mockCollection.toArray.mockResolvedValueOnce([{ title: 'Material 1' }]); // materials
      mockCollection.toArray.mockResolvedValueOnce([{ amount: 10 }]); // purchases
      mockCollection.toArray.mockResolvedValueOnce([{ eventName: 'login' }]); // analytics

      const exported = await exportData(user);
      
      expect(exported.profile).not.toHaveProperty('_id');
      expect(exported.profile.fullName).toBe('Test User');
      expect(exported.materials.length).toBe(1);
      expect(exported.purchases.length).toBe(1);
      expect(exported.analytics.length).toBe(1);
      expect(exported.version).toBe('1.0');
    });
  });

  describe('initiateDeletion', () => {
    it('blocks deletion if user has a legal hold', async () => {
      const user = { _id: '123', email: 'test@test.com', legalHold: true };
      
      await expect(initiateDeletion(user)).rejects.toThrow('Active legal hold prevents deletion.');
    });

    it('executes deletion workflow and unpins IPFS content', async () => {
      const user = { _id: '123', email: 'test@test.com', walletAddress: 'G123' };
      
      // Mock materials with an IPFS CID
      mockCollection.toArray.mockResolvedValueOnce([
        { _id: 'm1', ipfsCid: 'QmTestHash' }
      ]);

      const result = await initiateDeletion(user);
      
      expect(result).toBe(true);
      
      // Verify unpin was called
      expect(pinata.unpin).toHaveBeenCalledWith(['QmTestHash']);
      
      // Verify hard deletes
      expect(mockCollection.deleteOne).toHaveBeenCalledWith({ _id: 'm1' });
      
      // Verify pseudonymization (wallet linkage removed)
      expect(mockCollection.updateMany).toHaveBeenCalledWith(
        { buyer: 'G123' },
        expect.objectContaining({ $set: expect.objectContaining({ buyer: expect.any(String) }) })
      );
      
      // Verify analytics deletion
      expect(mockCollection.deleteMany).toHaveBeenCalledWith({ userId: 'G123' });
      
      // Verify tombstone creation
      expect(mockCollection.insertOne).toHaveBeenCalledWith(
        expect.objectContaining({ status: 'completed' })
      );
    });
  });
});
