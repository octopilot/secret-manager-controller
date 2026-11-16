{{- define "smc.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "smc.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else if contains (include "smc.name" .) .Release.Name -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name (include "smc.name" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}

{{- define "smc.selectorLabels" -}}
app.kubernetes.io/name: {{ include "smc.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "smc.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" }}
{{ include "smc.selectorLabels" . }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}

{{- define "smc.serviceAccountName" -}}
{{- default (include "smc.fullname" .) .Values.serviceAccount.name -}}
{{- end -}}

{{/* The ConfigMap name is fixed: the controller reads and watches it by this name. */}}
{{- define "smc.configMapName" -}}
secret-manager-controller-config
{{- end -}}

{{- define "smc.image" -}}
{{- if .Values.image.digest -}}
{{- printf "%s@%s" .Values.image.repository .Values.image.digest -}}
{{- else -}}
{{- /* Image tags are vX.Y.Z. appVersion is X.Y.Z in the repository, and the release
     pipeline (op) rewrites it to the tag, vX.Y.Z, in the published chart: accept both. */ -}}
{{- printf "%s:%s" .Values.image.repository (default (printf "v%s" (trimPrefix "v" .Chart.AppVersion)) .Values.image.tag) -}}
{{- end -}}
{{- end -}}
