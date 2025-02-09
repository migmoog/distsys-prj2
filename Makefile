all: comp1

j.%: prj2
	docker compose -f jeremy_tests/$*.yml up

comp%: prj2
	docker compose -f testcases/docker-compose-testcase-$*.yml up

prj2:
	docker build . -t $@

.PHONY: teardown%
teardown%:
	docker compose -f testcases/docker-compose-testcase-$*.yml down
