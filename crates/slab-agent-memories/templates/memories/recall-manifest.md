Memory manifest (newest first):
{% for line in lines %}- {{ line.filename }} | {{ line.title }} | keywords: {{ line.keywords }} | cwd: {{ line.cwd }} | {{ line.freshness }}
{% endfor %}
Workspace: {{ cwd }}

User request:
{{ input_message }}

Reply ONLY with JSON: {% raw %}{"filenames": [...]}{% endraw %} listing up to {{ top_k }} manifest filenames most relevant to the request, most relevant first. Use EXACT filenames from the manifest; if none are relevant reply with an empty list.