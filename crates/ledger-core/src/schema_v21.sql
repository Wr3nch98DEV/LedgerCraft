-- v21: customer deposits on sales orders (QuickBooks): a prepayment received for an order, applied
-- to the order's invoice when it's approved.
CREATE TABLE order_deposits (
  order_id INTEGER NOT NULL REFERENCES documents(id),
  bank_transaction_id INTEGER NOT NULL REFERENCES bank_transactions(id),
  PRIMARY KEY (order_id, bank_transaction_id)
);
