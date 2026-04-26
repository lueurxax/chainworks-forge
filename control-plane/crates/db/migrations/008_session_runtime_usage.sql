ALTER TABLE session_generations
  ADD COLUMN latest_cached_input_tokens INTEGER;

ALTER TABLE session_generations
  ADD COLUMN latest_output_tokens INTEGER;

ALTER TABLE session_generations
  ADD COLUMN latest_model_context_window INTEGER;
