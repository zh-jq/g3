
BEGIN {}

$1 ~ "^#.*" {
	next;
}

{
	print "    "$1", /* "$5" */";
}

END {}

