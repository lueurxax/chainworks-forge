-- P084: Preserve contract-derived rollback disposition for operator readback.

ALTER TABLE rollout_contract_checks
  ADD COLUMN rollback_disposition_json TEXT NOT NULL DEFAULT '{"mode":"not_applicable","data_loss_risk":"none","steps":[]}';
