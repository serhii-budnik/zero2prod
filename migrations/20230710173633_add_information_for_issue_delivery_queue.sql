ALTER TABLE issue_delivery_queue ADD COLUMN n_retries smallint NOT NULL DEFAULT 20;
ALTER TABLE issue_delivery_queue ADD COLUMN execute_after_in_secs integer NULL;
