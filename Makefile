all: comp1

comp%: prj2
	docker compose -f testcases/docker-compose-testcase-$*.yml up

prj2:
	docker build . -t $@

