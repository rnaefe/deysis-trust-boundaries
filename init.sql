CREATE TABLE IF NOT EXISTS work_items (
  id uuid PRIMARY KEY,
  payload jsonb NOT NULL,
  status text NOT NULL CHECK (status IN ('queued', 'running', 'complete', 'failed')),
  available_at timestamptz NOT NULL,
  attempts integer NOT NULL DEFAULT 0,
  last_error text,
  claimed_at timestamptz
);
