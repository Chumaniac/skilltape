use strict;
use warnings;
use JSON::PP;
use Encode qw(decode FB_CROAK);
use Getopt::Long qw(GetOptions);

my ($input, $output);
GetOptions('input=s' => \$input, 'output-dir=s' => \$output) or exit 1;
my $ok = eval {
    die 'arguments' unless defined($input) && defined($output);
    open my $source, '<:raw', $input or die 'input';
    my $body = '';
    while (length($body) < 1024 * 1024 + 1) {
        my $read = read($source, my $part, 1024 * 1024 + 1 - length($body));
        die 'input' unless defined $read;
        last unless $read;
        $body .= $part;
    }
    close $source;
    die 'capacity' if length($body) > 1024 * 1024;
    my $text = decode('UTF-8', $body, FB_CROAK);
    my @lines = split(/\r?\n/, $text, 1003);
    die 'header' unless shift(@lines) eq 'order_id,day,amount_cents';
    my (%seen, %days);
    my ($orders, $total) = (0, 0);
    for my $line (@lines) {
        next if $line eq '' && $orders > 0;
        die 'capacity' if $orders >= 1000 || length($line) > 256;
        my @values = split(/,/, $line, -1);
        die 'record' unless @values == 3;
        my ($id, $day, $cents) = @values;
        die 'record' unless $id =~ /^[A-Z0-9_-]{1,64}$/ && !$seen{$id} &&
            $day =~ /^[0-9]{4}-[0-9]{2}-[0-9]{2}$/ && $cents =~ /^[0-9]{1,16}$/;
        $cents = 0 + $cents;
        die 'amount' if $cents > 9007199254740991 || $total + $cents > 9007199254740991;
        $seen{$id} = 1;
        $days{$day}{orders} += 1; $days{$day}{total_cents} += $cents;
        $orders += 1; $total += $cents;
    }
    die 'empty input' unless $orders;
    mkdir $output, 0700 or die 'output' unless -d $output;
    die 'existing output' if -e "$output/metrics.csv" || -e "$output/summary.json";
    open my $csv, '>:encoding(UTF-8)', "$output/metrics.csv" or die 'output';
    print $csv "day,orders,total_cents\n";
    for my $day (sort keys %days) {
        printf $csv "%s,%.0f,%.0f\n", $day, $days{$day}{orders}, $days{$day}{total_cents};
    }
    close $csv or die 'output';
    open my $json, '>:encoding(UTF-8)', "$output/summary.json" or die 'output';
    print $json JSON::PP->new->canonical->pretty->encode({schema_version => 1,
        row_count => scalar(keys %days), total_orders => $orders, total_cents => $total});
    close $json or die 'output';
    1;
};
if (!$ok) { print STDERR "order-summary input or output is invalid\n"; exit 1; }
