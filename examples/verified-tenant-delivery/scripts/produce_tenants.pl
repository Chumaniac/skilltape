use strict;
use warnings;
use Config;
use Encode qw(decode FB_CROAK);
use Fcntl qw(O_RDONLY O_WRONLY O_CREAT O_EXCL O_NOFOLLOW O_NONBLOCK);
use Getopt::Long qw(GetOptions);
use JSON::PP;

my ($orders, $shipments, $output);
my ($observed_bytes, $observed_rows) = (0, 0);

sub read_material {
    my ($path, $unique) = @_;
    sysopen my $source, $path, O_RDONLY | O_NOFOLLOW | O_NONBLOCK or die 'input';
    die 'type' unless -f $source;
    my @before = stat($source);
    my $size = $before[7];
    die 'capacity' if $size > 1024 * 1024 - $observed_bytes;
    my $body = '';
    while (length($body) < $size) {
        my $remaining = $size - length($body);
        my $read = read($source, my $part, $remaining < 8192 ? $remaining : 8192);
        die 'input' unless defined($read) && $read > 0;
        $body .= $part;
    }
    my $probe = read($source, my $extra, 1);
    die 'changed' unless defined($probe) && $probe == 0;
    my @after = stat($source);
    die 'changed' if join(':', @before[0, 1, 2, 7, 9, 10]) ne join(':', @after[0, 1, 2, 7, 9, 10]);
    close $source or die 'input';
    $observed_bytes += length($body);
    my $text = decode('UTF-8', $body, FB_CROAK);
    my @lines = split(/\r?\n/, $text, -1);
    pop @lines if @lines && $lines[-1] eq '';
    die 'header' unless @lines && shift(@lines) eq 'tenant_id,order_id,amount_cents';
    die 'empty' unless @lines;
    my %groups;
    my $total = 0;
    for my $line (@lines) {
        die 'capacity' if ++$observed_rows > 1000 || length($line) > 256;
        my @values = split(/,/, $line, -1);
        die 'record' unless @values == 3;
        my ($tenant, $id, $amount) = @values;
        die 'record' unless $tenant =~ /^[A-Z0-9_-]{1,64}$/ && $id =~ /^[A-Z0-9_-]{1,64}$/ && $amount =~ /^[0-9]{1,16}$/;
        die 'duplicate' if $unique && exists $groups{$tenant}{$id};
        $amount = 0 + $amount;
        die 'amount' if $amount > 9007199254740991 || $total + $amount > 9007199254740991;
        $total += $amount;
        # Nested identity maps avoid delimiter or adjacent-string collisions.
        $groups{$tenant}{$id} = ($groups{$tenant}{$id} // 0) + $amount;
    }
    my $csv = "tenant_id,order_id,amount_cents\n";
    my $count = 0;
    for my $tenant (sort keys %groups) {
        for my $id (sort keys %{$groups{$tenant}}) {
            $csv .= sprintf "%s,%s,%.0f\n", $tenant, $id, $groups{$tenant}{$id};
            ++$count;
        }
    }
    return ($csv, $count, $total);
}

my $ok = eval {
    my $options;
    {
        local $SIG{__WARN__} = sub { };
        $options = GetOptions('orders=s' => \$orders, 'shipments=s' => \$shipments, 'output-dir=s' => \$output);
    }
    die 'arguments' unless $options;
    die 'arguments' unless defined($orders) && defined($shipments) && defined($output) && !@ARGV;
    die 'platform' unless $Config{ivsize} >= 8;
    my ($expected, $expected_rows) = read_material($orders, 1);
    my ($allocated, $allocation_rows, $total_cents) = read_material($shipments, 0);
    die 'output' if -l $output;
    mkdir $output, 0700 or die 'output' unless -d $output;
    my %bodies = ('expected.csv' => $expected, 'allocations.csv' => $allocated,
        'summary.json' => JSON::PP->new->canonical->pretty->encode({expected_rows => $expected_rows,
            allocation_rows => $allocation_rows, total_cents => $total_cents}));
    for my $name (keys %bodies) {
        die 'existing output' if -e "$output/$name" || -l "$output/$name";
    }
    for my $name (sort keys %bodies) {
        sysopen my $target, "$output/$name", O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, 0600 or die 'output';
        print {$target} $bodies{$name} or die 'output';
        close $target or die 'output';
    }
    1;
};
if (!$ok) { print STDERR "tenant-delivery input or output is invalid\n"; exit 1; }
