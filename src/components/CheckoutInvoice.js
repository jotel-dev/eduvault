import React, { useState, useEffect } from 'react';
import { validateCheckoutBalance } from '@/lib/stellar/checkoutService';

export default function CheckoutInvoice({ walletAddress, totalPrice, estimatedGas, onConfirm }) {
  const [balanceStatus, setBalanceStatus] = useState(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function checkBalance() {
      if (!walletAddress) {
        setLoading(false);
        return;
      }
      setLoading(true);
      try {
        const status = await validateCheckoutBalance({ walletAddress, totalPrice, estimatedGas });
        setBalanceStatus(status);
      } catch (err) {
        console.error("Failed to check balance", err);
      } finally {
        setLoading(false);
      }
    }
    checkBalance();
  }, [walletAddress, totalPrice, estimatedGas]);

  const canCheckout = balanceStatus?.hasEnough;

  return (
    <div className="checkout-invoice p-4 border rounded">
      <h3 className="text-lg font-bold mb-4">Checkout Invoice</h3>
      <div className="flex justify-between mb-2">
        <span>Item Price:</span>
        <span>{totalPrice} XLM</span>
      </div>
      <div className="flex justify-between mb-4">
        <span>Estimated Gas:</span>
        <span>{estimatedGas} XLM</span>
      </div>
      <div className="flex justify-between font-bold border-t pt-2 mb-4">
        <span>Total Required:</span>
        <span>{totalPrice + estimatedGas} XLM</span>
      </div>

      {!loading && balanceStatus && !canCheckout && (
        <div className="bg-red-50 border-l-4 border-red-500 p-4 mb-4 text-red-700">
          <p className="font-bold">Insufficient Balance</p>
          <p>Your remaining balance ({balanceStatus.remainingBalance} XLM) is negative.</p>
          <p className="text-sm mt-2">
            Please fund your wallet via Friendbot or deposit more XLM to cover the total cost and gas fees before initiating checkout.
          </p>
        </div>
      )}

      <button 
        className={`w-full py-2 rounded font-bold text-white ${canCheckout ? 'bg-blue-600 hover:bg-blue-700' : 'bg-gray-400 cursor-not-allowed'}`}
        disabled={!canCheckout || loading}
        onClick={onConfirm}
      >
        {loading ? 'Checking Balance...' : 'Initiate Checkout'}
      </button>
    </div>
  );
}
