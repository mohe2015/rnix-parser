''
    for i in /usr /sw /opt /pkg; do	# improve purity
      substituteInPlace ./setup.py --replace $i /no-such-path
    done
  ''