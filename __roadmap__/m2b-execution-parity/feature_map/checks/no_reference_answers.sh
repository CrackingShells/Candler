#!/bin/sh
# S3.R17: the repo's reference answers (examples/orchestration_demo, mappings/, bio-opentrons scripts, VCS/roadmap dirs) are neither in the work dir nor anywhere in the container.
: "${EXEC:?}"; : "${WORKSPACE_DIR:?}"
hit=$(find "$WORKSPACE_DIR" -not -path '*/.venv/*' -not -path '*/site-packages/*' \( -name orchestration_demo -o -name mappings -o -name ingest_bio_opentrons.py -o -name test_bio_planning.py -o -name .git -o -name __reports__ -o -name __roadmap__ -o -name plan-browser-react \) -print -quit)
[ -z "$hit" ] || { echo "FAIL no_reference_answers: $hit in work dir"; exit 1; }
hit=$($EXEC sh -c "find / -not -path '/proc/*' -not -path '/sys/*' -not -path '/usr/*' -not -path '*/.venv/*' -not -path '*/site-packages/*' \\( -name orchestration_demo -o -name mappings -o -name ingest_bio_opentrons.py -o -name test_bio_planning.py -o -name .git -o -name __reports__ -o -name __roadmap__ -o -name plan-browser-react \\) -print -quit 2>/dev/null")
[ -z "$hit" ] || { echo "FAIL no_reference_answers: $hit in agent container"; exit 1; }
echo "OK no_reference_answers"
