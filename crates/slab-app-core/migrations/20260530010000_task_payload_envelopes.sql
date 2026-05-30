UPDATE tasks
SET result_data = CASE
    WHEN json_valid(result_data) THEN json_object(
        'kind', 'task_result',
        'version', 1,
        'data', json(result_data)
    )
    ELSE json_object(
        'kind', 'task_result',
        'version', 1,
        'data', result_data
    )
END
WHERE result_data IS NOT NULL
  AND CASE
      WHEN json_valid(result_data) THEN NOT (
          json_extract(result_data, '$.kind') = 'task_result'
          AND json_extract(result_data, '$.version') = 1
          AND json_type(result_data, '$.data') IS NOT NULL
      )
      ELSE 1
  END;
