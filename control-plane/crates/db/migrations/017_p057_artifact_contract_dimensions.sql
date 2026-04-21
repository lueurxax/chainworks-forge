ALTER TABLE artifact_contract_generations
  ADD COLUMN canonical_dimensions_json TEXT NOT NULL DEFAULT '{}';
