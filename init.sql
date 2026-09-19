CREATE TABLE IF NOT EXISTS work_items (
  id uuid PRIMARY KEY,
  payload jsonb NOT NULL,
  status text NOT NULL CHECK (status IN ('queued', 'running', 'complete', 'failed')),
  available_at timestamptz NOT NULL,
  attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
  max_attempts integer NOT NULL DEFAULT 5 CHECK (max_attempts > 0),
  last_error text,
  claimed_at timestamptz,
  lease_token uuid,
  lease_expires_at timestamptz,
  CONSTRAINT running_lease_consistency CHECK (
    (status = 'running' AND lease_token IS NOT NULL AND lease_expires_at IS NOT NULL)
    OR (status <> 'running' AND lease_token IS NULL AND lease_expires_at IS NULL)
  )
);

CREATE INDEX IF NOT EXISTS work_items_ready_idx
  ON work_items (available_at, id) WHERE status = 'queued';

CREATE INDEX IF NOT EXISTS work_items_expired_lease_idx
  ON work_items (lease_expires_at) WHERE status = 'running';
