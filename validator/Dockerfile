# The official HL7 FHIR validator, plus a script that converts documents through the service
# and validates the result against FHIR R4 and US Core.
FROM eclipse-temurin:21-jre
ARG VALIDATOR_VERSION=7.0.3
RUN apt-get update && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/* \
    && curl -fsSL -o /opt/validator_cli.jar \
       "https://github.com/hapifhir/org.hl7.fhir.core/releases/download/${VALIDATOR_VERSION}/validator_cli.jar"
COPY validate.sh /usr/local/bin/validate-documents
ENTRYPOINT ["/usr/local/bin/validate-documents"]
