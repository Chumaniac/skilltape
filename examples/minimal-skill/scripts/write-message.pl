use strict;
use warnings;
use File::Path qw(make_path);
make_path('fixtures/output');
open my $output, '>:raw', 'fixtures/output/message.txt' or die "output unavailable";
print {$output} "hello from SkillTape\n" or die "output write failed";
close $output or die "output close failed";
